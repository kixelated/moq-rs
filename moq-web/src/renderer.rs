use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use wasm_bindgen::{prelude::Closure, JsCast};
use web_codecs::{VideoDecoded, VideoFrame};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

use crate::Result;

pub struct Renderer {
	decoded: VideoDecoded,
	animate: RenderAnimate,
}

impl Renderer {
	pub fn new(decoded: VideoDecoded) -> Self {
		Self {
			animate: RenderAnimate::new(None),
			decoded,
		}
	}

	pub fn canvas(&mut self, canvas: Option<HtmlCanvasElement>) {
		self.animate.state.borrow_mut().canvas = canvas;
	}

	pub async fn run(&mut self) -> Result<()> {
		while let Some(frame) = self.decoded.next().await? {
			self.animate.push(frame);
		}

		Ok(())
	}
}

#[derive(Clone)]
struct RenderAnimate {
	state: Rc<RefCell<RenderState>>,
}

struct RenderState {
	scheduled: bool,
	canvas: Option<HtmlCanvasElement>,
	queue: VecDeque<VideoFrame>,
	render: Option<Closure<dyn FnMut()>>,
}

impl RenderAnimate {
	pub fn new(canvas: Option<HtmlCanvasElement>) -> Self {
		let state = Rc::new(RefCell::new(RenderState {
			scheduled: false,
			canvas,
			queue: Default::default(),
			render: None,
		}));

		let this = Self { state };

		let mut cloned = this.clone();
		let f = Closure::wrap(Box::new(move || {
			cloned.render();
		}) as Box<dyn FnMut()>);

		this.state.borrow_mut().render = Some(f);
		this
	}

	pub fn push(&mut self, frame: VideoFrame) {
		let mut state = self.state.borrow_mut();
		state.queue.push_back(frame);
		drop(state);

		self.schedule();
	}

	fn render(&mut self) {
		let mut state = self.state.borrow_mut();
		state.scheduled = false;

		let frame = match state.queue.pop_front() {
			Some(frame) => frame,
			None => return,
		};

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
	}

	fn schedule(&mut self) {
		let mut state = self.state.borrow_mut();
		if state.scheduled {
			return;
		}

		let render = state.render.as_ref().unwrap();
		request_animation_frame(render);

		state.scheduled = true;
	}
}

fn request_animation_frame(f: &Closure<dyn FnMut()>) {
	web_sys::window()
		.unwrap()
		.request_animation_frame(f.as_ref().unchecked_ref())
		.expect("should register `requestAnimationFrame` on Window");
}
