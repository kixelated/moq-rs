use std::{cell::RefCell, collections::VecDeque, rc::Rc, time::Duration};

use moq_karp::Dimensions;
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::spawn_local;
use web_codecs::{Timestamp, VideoFrame};
use web_sys::{OffscreenCanvas, OffscreenCanvasRenderingContext2d};
use web_time::Instant;

use super::{ControlsRecv, RendererStatus, StatusSend};

struct Render {
	status: StatusSend,

	state: RendererStatus,
	scheduled: bool,
	resolution: Dimensions,

	// Used to determine which frame to render next.
	latency: Duration,
	latency_ref: Option<(Instant, Timestamp)>,

	canvas: Option<OffscreenCanvas>,
	context: Option<OffscreenCanvasRenderingContext2d>,
	queue: VecDeque<VideoFrame>,
	draw: Option<Closure<dyn FnMut()>>,

	// We keep triggering a 1s setTimeout to detect buffering.
	timeout: Option<Closure<dyn FnMut()>>,
	timeout_handle: Option<i32>,
}

impl Render {
	pub fn new(status: StatusSend) -> Self {
		Self {
			status,
			scheduled: false,
			state: RendererStatus::Idle,
			canvas: None,
			context: None,
			resolution: Default::default(),
			latency: Default::default(),
			latency_ref: None,
			queue: Default::default(),
			draw: None,
			timeout: None,
			timeout_handle: None,
		}
	}

	fn duration(&self) -> Option<Duration> {
		Some(
			self.queue
				.back()?
				.timestamp()
				.saturating_sub(self.queue.front()?.timestamp()),
		)
	}

	pub fn push(&mut self, frame: VideoFrame) {
		self.queue.push_back(frame);
		self.trim_buffer();
		self.schedule();
	}

	pub fn draw(&mut self) {
		self.scheduled = false;

		match self.state {
			RendererStatus::Paused | RendererStatus::Idle => return,
			RendererStatus::Live | RendererStatus::Buffering => (),
		}

		let now = Instant::now();

		let mut frame = self.queue.pop_front().expect("rendered with no frames");

		if let Some((wall_ref, pts_ref)) = self.latency_ref {
			let wall_elapsed = now - wall_ref;

			while let Some(next) = self.queue.front() {
				let pts_elapsed = next.timestamp().saturating_sub(pts_ref);
				if wall_elapsed <= pts_elapsed {
					break;
				}

				frame = self.queue.pop_front().unwrap();

				// We know we're live because we're dropping unique frames.
				self.set_live();
			}
		} else {
			// This is the first frame, render it.
			self.latency_ref = Some((now + self.latency, frame.timestamp()));
		}

		if let Some(context) = &mut self.context {
			context.draw_image_with_video_frame(frame.inner(), 0.0, 0.0).unwrap();
		}

		// Add the frame back for consideration unless the buffer is too full.
		if self.duration().unwrap_or_default() < self.latency {
			self.queue.push_front(frame);
		} else {
			// We know we're live because we're dropping unique frames.
			self.set_live();
		}

		// Schedule the next frame.
		self.schedule();
	}

	fn trim_buffer(&mut self) {
		if self.queue.is_empty() {
			self.latency_ref = None;
			return;
		}

		// Drop frames if the buffer is too full.
		while self.duration().unwrap() > self.latency {
			self.latency_ref = None;
			self.queue.pop_front();
		}
	}

	pub fn schedule(&mut self) {
		if self.scheduled {
			return;
		}

		match self.state {
			RendererStatus::Live | RendererStatus::Buffering => (),
			_ => return,
		}

		if self.queue.is_empty() {
			return;
		}

		let draw = self.draw.as_ref().unwrap();
		request_animation_frame(draw);

		self.scheduled = true;
	}

	pub fn set_canvas(&mut self, canvas: Option<OffscreenCanvas>) {
		let canvas = match canvas {
			Some(canvas) => canvas,
			None => {
				self.canvas = None;
				self.context = None;
				return;
			}
		};

		// Tell the browser that we're not going to use the alpha channel for better performance.
		// We need to create a JsValue until web_sys implements a proper way to create the options.
		// let options = { alpha: false };
		let options = js_sys::Object::new();
		js_sys::Reflect::set(&options, &"alpha".into(), &false.into()).unwrap();

		let ctx: web_sys::OffscreenCanvasRenderingContext2d = canvas
			.get_context_with_context_options("2d", &options)
			.unwrap()
			.unwrap()
			.unchecked_into();

		self.context = Some(ctx);
		self.canvas = Some(canvas);
	}

	pub fn set_paused(&mut self, paused: bool) {
		match paused {
			true => {
				self.queue.clear();
				self.latency_ref = None;
				self.set_state(RendererStatus::Paused);
			}
			false => {
				self.set_state(RendererStatus::Buffering);
				self.schedule();
			}
		};
	}

	pub fn set_resolution(&mut self, resolution: Dimensions) {
		self.resolution = resolution;

		if let Some(canvas) = self.canvas.as_mut() {
			canvas.set_width(resolution.width);
			canvas.set_height(resolution.height);
		}

		if resolution == Default::default() {
			self.set_state(RendererStatus::Idle);
			self.queue.clear();
		} else {
			self.set_state(RendererStatus::Buffering);
			self.schedule();
		}
	}

	pub fn set_latency(&mut self, duration: Duration) {
		self.latency = duration;
		self.latency_ref = None;
		self.set_state(RendererStatus::Buffering);
	}

	fn set_state(&mut self, state: RendererStatus) {
		self.state = state;
		self.status.render.update(state);
	}

	fn set_live(&mut self) {
		self.set_state(RendererStatus::Live);

		// Cancel any existing timeout.
		if let Some(handle) = self.timeout_handle {
			cancel_timeout(handle);
		}

		// Set up a timeout to mark the stream as buffering after 1s
		let timeout = self.timeout.as_ref().unwrap();
		self.timeout_handle = set_timeout(timeout, Duration::from_secs(1)).into();
	}

	// Called after 1s of no frames.
	fn timeout(&mut self) {
		if self.state == RendererStatus::Live {
			self.set_state(RendererStatus::Buffering);
		}
	}
}

#[derive(Clone)]
pub struct Renderer {
	state: Rc<RefCell<Render>>,
}

impl Renderer {
	pub fn new(controls: ControlsRecv, status: StatusSend) -> Self {
		let render = Rc::new(RefCell::new(Render::new(status)));
		let render2 = render.clone();
		let render3 = render.clone();

		render.borrow_mut().draw = Closure::wrap(Box::new(move || {
			render2.borrow_mut().draw();
		}) as Box<dyn FnMut()>)
		.into();

		render.borrow_mut().timeout = Closure::wrap(Box::new(move || {
			render3.borrow_mut().timeout();
		}) as Box<dyn FnMut()>)
		.into();

		let this = Self { state: render };
		let this2 = this.clone();
		spawn_local(async move { this2.run(controls).await });

		this
	}

	async fn run(&self, mut controls: ControlsRecv) {
		loop {
			tokio::select! {
				Some(paused) = controls.paused.next() => {
					self.state.borrow_mut().set_paused(paused);
				},
				Some(latency) = controls.latency.next() => {
					self.state.borrow_mut().set_latency(latency);
				},
				Some(canvas) = controls.canvas.next() => {
					self.state.borrow_mut().set_canvas(canvas);
				},
				else => break,
			}
		}
	}

	pub fn set_resolution(&self, resolution: Dimensions) {
		self.state.borrow_mut().set_resolution(resolution);
	}

	pub fn push(&mut self, frame: VideoFrame) {
		self.state.borrow_mut().push(frame);
	}
}

// Based on: https://rustwasm.github.io/wasm-bindgen/examples/request-animation-frame.html
// But with Worker support, which could contribute back to gloo_render.
fn request_animation_frame(f: &Closure<dyn FnMut()>) {
	let global = js_sys::global();
	if let Some(window) = global.dyn_ref::<web_sys::Window>() {
		// Main thread
		window
			.request_animation_frame(f.as_ref().unchecked_ref())
			.expect("should register `requestAnimationFrame` on Window");
	} else if let Some(worker) = global.dyn_ref::<web_sys::DedicatedWorkerGlobalScope>() {
		// Dedicated Worker
		worker
			.request_animation_frame(f.as_ref().unchecked_ref())
			.expect("should register `requestAnimationFrame` on DedicatedWorkerGlobalScope");
	} else {
		unimplemented!("Unsupported context: neither Window nor DedicatedWorkerGlobalScope");
	}
}

fn set_timeout(f: &Closure<dyn FnMut()>, timeout: Duration) -> i32 {
	let global = js_sys::global();
	if let Some(window) = global.dyn_ref::<web_sys::Window>() {
		// Main thread
		window
			.set_timeout_with_callback_and_timeout_and_arguments_0(
				f.as_ref().unchecked_ref(),
				timeout.as_millis() as i32,
			)
			.expect("should register `setTimeout` on Window")
	} else if let Some(worker) = global.dyn_ref::<web_sys::DedicatedWorkerGlobalScope>() {
		// Dedicated Worker
		worker
			.set_timeout_with_callback_and_timeout_and_arguments_0(
				f.as_ref().unchecked_ref(),
				timeout.as_millis() as i32,
			)
			.expect("should register `setTimeout` on DedicatedWorkerGlobalScope")
	} else {
		unimplemented!("Unsupported context: neither Window nor DedicatedWorkerGlobalScope");
	}
}

fn cancel_timeout(handle: i32) {
	let global = js_sys::global();
	if let Some(window) = global.dyn_ref::<web_sys::Window>() {
		// Main thread
		window.clear_timeout_with_handle(handle);
	} else if let Some(worker) = global.dyn_ref::<web_sys::DedicatedWorkerGlobalScope>() {
		// Dedicated Worker
		worker.clear_timeout_with_handle(handle);
	} else {
		unimplemented!("Unsupported context: neither Window nor DedicatedWorkerGlobalScope");
	}
}
