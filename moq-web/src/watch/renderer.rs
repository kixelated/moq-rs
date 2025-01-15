use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use wasm_bindgen::{prelude::Closure, JsCast};
use web_codecs::VideoFrame;
use web_sys::OffscreenCanvas;

struct State {
	scheduled: bool,
	canvas: Option<OffscreenCanvas>,
	queue: VecDeque<VideoFrame>,
	draw: Option<Closure<dyn FnMut()>>,
}

#[derive(Clone)]
pub struct Renderer {
	state: Rc<RefCell<State>>,
}

impl Renderer {
	pub fn new() -> Self {
		let state = Rc::new(RefCell::new(State {
			scheduled: false,
			canvas: None,
			queue: Default::default(),
			draw: None,
		}));

		let this = Self { state };

		let mut cloned = this.clone();
		let f = Closure::wrap(Box::new(move || {
			cloned.draw();
		}) as Box<dyn FnMut()>);

		this.state.borrow_mut().draw = Some(f);
		this
	}

	pub fn render(&mut self, frame: VideoFrame) {
		let mut state = self.state.borrow_mut();
		state.queue.push_back(frame);
		drop(state);

		self.schedule();
	}

	fn draw(&mut self) {
		let mut state = self.state.borrow_mut();
		state.scheduled = false;

		let frame = state.queue.pop_front().unwrap();

		let canvas = match &mut state.canvas {
			Some(canvas) => canvas,
			None => return,
		};

		// TODO don't change the canvas size?
		canvas.set_width(frame.display_width());
		canvas.set_height(frame.display_height());

		// Tell the browser that we're not going to use the alpha channel for better performance.
		// We need to create a JsValue until web_sys implements a proper way to create the options.
		// let options = { alpha: false };
		let options = js_sys::Object::new();
		js_sys::Reflect::set(&options, &"alpha".into(), &false.into()).unwrap();

		let ctx = canvas
			.get_context_with_context_options("2d", &options)
			.unwrap()
			.unwrap();

		if let Some(ctx) = ctx.dyn_ref::<web_sys::OffscreenCanvasRenderingContext2d>() {
			ctx.draw_image_with_video_frame(frame.inner(), 0.0, 0.0).unwrap();
		}

		drop(state);

		// Schedule the next frame.
		self.schedule();
	}

	fn schedule(&mut self) {
		let mut state = self.state.borrow_mut();
		if state.scheduled {
			return;
		}

		if state.queue.is_empty() {
			return;
		}

		let draw = state.draw.as_ref().unwrap();
		request_animation_frame(draw);

		state.scheduled = true;
	}

	pub fn canvas(&mut self, canvas: Option<OffscreenCanvas>) {
		self.state.borrow_mut().canvas = canvas;
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
