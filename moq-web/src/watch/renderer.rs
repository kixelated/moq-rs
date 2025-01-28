use std::{cell::RefCell, collections::VecDeque, rc::Rc, time::Duration};

use moq_karp::Dimensions;
use wasm_bindgen::{prelude::Closure, JsCast};
use web_codecs::{Timestamp, VideoFrame};
use web_sys::{OffscreenCanvas, OffscreenCanvasRenderingContext2d};
use web_time::Instant;

struct Inner {
	scheduled: bool,
	paused: bool,
	resolution: Dimensions,

	// Used to determine which frame to render next.
	latency: Duration,
	latency_max: Duration,
	latency_ref: Option<(Instant, Timestamp)>,

	canvas: Option<OffscreenCanvas>,
	context: Option<OffscreenCanvasRenderingContext2d>,
	queue: VecDeque<VideoFrame>,
	draw: Option<Closure<dyn FnMut()>>,
}

impl Inner {
	pub fn new() -> Self {
		Self {
			scheduled: false,
			paused: false,
			canvas: None,
			context: None,
			resolution: Default::default(),
			latency: Default::default(),
			latency_max: Duration::from_secs(10),
			latency_ref: None,
			queue: Default::default(),
			draw: None,
		}
	}

	fn duration(&self) -> Option<Duration> {
		Some(self.queue.back()?.timestamp() - self.queue.front()?.timestamp())
	}

	pub fn push(&mut self, frame: VideoFrame) {
		self.queue.push_back(frame);
		self.trim_buffer();

		if !self.paused {
			self.schedule();
		}
	}

	pub fn draw(&mut self) {
		self.scheduled = false;

		let now = Instant::now();

		// We pop instead of using front().unwrap(), but we'll push the frame back when done.
		let mut frame = match self.queue.pop_front() {
			Some(frame) => frame,
			None => return,
		};

		if let Some((wall_ref, pts_ref)) = self.latency_ref {
			let wall_elapsed = now - wall_ref;

			while !self.queue.is_empty() {
				let pts_elapsed = frame.timestamp() - pts_ref;

				if wall_elapsed <= pts_elapsed {
					break;
				}

				frame = self.queue.pop_front().unwrap();
			}
		} else {
			self.latency_ref = Some((now + self.latency, frame.timestamp()));
		}

		if let Some(context) = &mut self.context {
			context.draw_image_with_video_frame(frame.inner(), 0.0, 0.0).unwrap();
		}

		// Add the frame back for consideration unless the buffer is too full.
		if frame.timestamp() + self.latency >= self.queue.back().map(|f| f.timestamp()).unwrap_or_default() {
			self.queue.push_front(frame);
		}

		// Schedule the next frame.
		self.schedule();
	}

	fn trim_buffer(&mut self) {
		if self.queue.is_empty() {
			self.latency_ref = None;
			return;
		}

		// Check if the buffer is too full.
		let mut duration = self.duration().unwrap();
		if duration > self.latency_max {
			tracing::warn!(dropped = ?(duration - self.latency_max), "full buffer");
			self.latency_ref = None;

			while duration > self.latency_max {
				self.queue.pop_front();
				duration = self.duration().unwrap();
			}
		}
	}

	pub fn schedule(&mut self) {
		if self.scheduled {
			return;
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

		let resolution = self.resolution;

		if let Some(canvas) = self.canvas.as_mut() {
			canvas.set_width(resolution.width);
			canvas.set_height(resolution.height);
		}

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
		self.queue.clear();
		self.latency_ref = None;
		self.paused = paused;
	}

	pub fn set_resolution(&mut self, resolution: Dimensions) {
		self.resolution = resolution;

		if let Some(canvas) = self.canvas.as_mut() {
			canvas.set_width(resolution.width);
			canvas.set_height(resolution.height);
		}
	}

	pub fn set_latency(&mut self, duration: Duration) {
		self.latency = duration;
		self.latency_ref = None;
	}

	pub fn set_latency_max(&mut self, duration: Duration) {
		self.latency_max = duration;
		self.trim_buffer();
	}
}

impl Default for Inner {
	fn default() -> Self {
		Self::new()
	}
}

#[derive(Clone)]
pub struct Renderer {
	state: Rc<RefCell<Inner>>,
}

impl Renderer {
	pub fn new() -> Self {
		let state = Rc::new(RefCell::new(Inner::default()));

		let cloned = state.clone();
		let f = Closure::wrap(Box::new(move || {
			cloned.borrow_mut().draw();
		}) as Box<dyn FnMut()>);

		let this = Self { state };
		this.state.borrow_mut().draw = Some(f);
		this
	}

	pub fn push(&mut self, frame: VideoFrame) {
		self.state.borrow_mut().push(frame);
	}

	pub fn set_canvas(&mut self, canvas: Option<OffscreenCanvas>) {
		self.state.borrow_mut().set_canvas(canvas);
	}

	pub fn set_paused(&mut self, paused: bool) {
		self.state.borrow_mut().set_paused(paused);
	}

	pub fn set_resolution(&mut self, resolution: Dimensions) {
		self.state.borrow_mut().set_resolution(resolution);
	}

	pub fn set_latency(&mut self, duration: Duration) {
		self.state.borrow_mut().set_latency(duration);
	}

	pub fn set_latency_max(&mut self, duration: Duration) {
		self.state.borrow_mut().set_latency_max(duration);
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
