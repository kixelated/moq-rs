use web_async::FuturesExt;

use crate::Result;

use std::{cell::RefCell, collections::VecDeque, rc::Rc, time::Duration};

use hang::Dimensions;
use wasm_bindgen::{prelude::*, JsCast};
use web_codecs::{Timestamp, VideoFrame};
use web_sys::{OffscreenCanvas, OffscreenCanvasRenderingContext2d};
use web_time::Instant;

pub struct Video {
	render: Rc<RefCell<Render>>,
	track: Option<VideoTrack>,

	broadcast: Option<hang::BroadcastConsumer>,
	catalog: Option<hang::Catalog>,
}

impl Video {
	pub fn new() -> Self {
		let render = Rc::new(RefCell::new(Render::new()));
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

		Self {
			render,
			track: None,
			broadcast: None,
			catalog: None,
		}
	}

	// Returns a useless Option so we can short-circuit if video is disabled.
	fn reinit(&mut self) -> Option<()> {
		let existing = self.track.take();

		let broadcast = self.broadcast.as_ref()?;
		let catalog = self.catalog.as_ref()?;

		let mut render = self.render.borrow_mut();
		if render.canvas.is_none() || !render.visible {
			return Some(());
		}

		// TODO select the video track based on
		let video = catalog.video.first()?;

		if let Some(existing) = existing {
			if existing.info.track == video.track {
				self.track = Some(existing);
				return Some(());
			}
		}

		let track = broadcast.track(&video.track);

		// TODO handle the error instead of ignoring it.
		let video = VideoTrack::new(track, video.clone()).ok()?;
		if let Some(resolution) = video.info.resolution {
			render.set_resolution(resolution);
		}

		tracing::info!(info = ?video.info, "loaded video track");
		self.track = Some(video);

		Some(())
	}

	pub async fn run(&mut self) {
		if let Err(err) = self.run_inner().await {
			tracing::error!(?err, "error running video; disabling");
		}

		// Prevent infinite loops by disabling the track.
		self.track.take();

		// Block indefinitely so we don't break out of the parent select! loop.
		// This is a hack as we're abusing select! to run tasks in parallel but ignore the results.
		std::future::pending::<()>().await;
	}

	async fn run_inner(&mut self) -> Result<()> {
		let track = match self.track.as_mut() {
			Some(track) => track,
			None => return Ok(()),
		};

		while let Some(frame) = track.frame().await? {
			self.render.borrow_mut().push(frame);
		}

		Ok(())
	}

	pub fn set_catalog(&mut self, broadcast: Option<hang::BroadcastConsumer>, catalog: Option<hang::Catalog>) {
		self.broadcast = broadcast;
		self.catalog = catalog;
		self.reinit();
	}

	pub fn set_canvas(&mut self, canvas: Option<OffscreenCanvas>) {
		self.render.borrow_mut().set_canvas(canvas);
		self.reinit();
	}

	pub fn set_paused(&mut self, paused: bool) {
		self.render.borrow_mut().set_paused(paused);
	}

	pub fn set_visible(&mut self, visible: bool) {
		self.render.borrow_mut().set_visible(visible);
		self.reinit();
	}

	pub fn set_latency(&mut self, duration: Duration) {
		self.render.borrow_mut().set_latency(duration);
	}
}

impl Default for Video {
	fn default() -> Self {
		Self::new()
	}
}

pub struct VideoTrack {
	pub track: hang::TrackConsumer,
	pub info: hang::Video,

	decoder: web_codecs::VideoDecoder,
	decoded: web_codecs::VideoDecoded,
}

impl VideoTrack {
	pub fn new(track: hang::TrackConsumer, info: hang::Video) -> Result<Self> {
		// Construct the video decoder
		let (decoder, decoded) = web_codecs::VideoDecoderConfig {
			codec: info.codec.to_string(),
			description: info.description.clone(),
			resolution: info.resolution.map(|r| web_codecs::Dimensions {
				width: r.width,
				height: r.height,
			}),
			latency_optimized: Some(true),
			..Default::default()
		}
		.build()?;

		Ok(Self {
			track,
			info,
			decoder,
			decoded,
		})
	}

	pub async fn frame(&mut self) -> Result<Option<web_codecs::VideoFrame>> {
		loop {
			tokio::select! {
				Some(frame) = self.track.read().transpose() => {
					let frame = frame?;

					let frame = web_codecs::EncodedFrame {
						payload: frame.payload,
						timestamp: frame.timestamp,
						keyframe: frame.keyframe,
					};

					self.decoder.decode(frame)?;
				},
				Some(frame) = self.decoded.next().transpose() => return Ok(Some(frame?)),
				else => return Ok(None),
			}
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum RendererStatus {
	Idle,
	Paused,
	Buffering,
	Live,
}

struct Render {
	state: RendererStatus,
	scheduled: bool,
	resolution: Dimensions,

	// Used to determine which frame to render next.
	latency: Duration,
	latency_ref: Option<(Instant, Timestamp)>,

	// Disable rendering when the video is not visible.
	visible: bool,

	canvas: Option<OffscreenCanvas>,
	context: Option<OffscreenCanvasRenderingContext2d>,
	queue: VecDeque<VideoFrame>,
	draw: Option<Closure<dyn FnMut()>>,

	// We keep triggering a 1s setTimeout to detect buffering.
	timeout: Option<Closure<dyn FnMut()>>,
	timeout_handle: Option<i32>,
}

impl Render {
	pub fn new() -> Self {
		Self {
			scheduled: false,
			state: RendererStatus::Idle,
			canvas: None,
			context: None,
			resolution: Default::default(),
			latency: Default::default(),
			latency_ref: None,
			queue: Default::default(),
			draw: None,
			visible: true,
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

	fn draw(&mut self) {
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
			context.draw_image_with_video_frame(&frame, 0.0, 0.0).unwrap();
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

		if !self.visible {
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

	pub fn set_visible(&mut self, visible: bool) {
		self.visible = visible;
		self.schedule();
	}

	fn set_state(&mut self, state: RendererStatus) {
		self.state = state;
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
