use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use wasm_bindgen::{prelude::Closure, JsCast};
use web_codecs::VideoFrame;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

struct State {
	scheduled: bool,
	canvas: Option<HtmlCanvasElement>,
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

		// TODO error handling lul
		let ctx = canvas.get_context("2d").unwrap().unwrap();

		if let Some(ctx) = ctx.dyn_ref::<CanvasRenderingContext2d>() {
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

	pub fn canvas(&mut self, canvas: Option<HtmlCanvasElement>) {
		self.state.borrow_mut().canvas = canvas;
	}
}

fn request_animation_frame(f: &Closure<dyn FnMut()>) {
	web_sys::window()
		.unwrap()
		.request_animation_frame(f.as_ref().unchecked_ref())
		.expect("should register `requestAnimationFrame` on Window");
}
