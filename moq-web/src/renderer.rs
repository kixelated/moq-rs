use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use wasm_bindgen::{prelude::Closure, JsCast};
use web_codecs::{VideoDecoded, VideoFrame};
use web_sys::{CanvasRenderingContext2d, OffscreenCanvas, OffscreenCanvasRenderingContext2d};

use crate::{Result, Run};

pub struct Renderer {
    decoded: VideoDecoded,
    animate: RenderAnimate,
}

impl Renderer {
    pub fn new(decoded: VideoDecoded, canvas: Option<OffscreenCanvas>) -> Self {
        Self {
            animate: RenderAnimate::new(canvas),
            decoded,
        }
    }

    pub fn update(&mut self, canvas: Option<OffscreenCanvas>) {
        self.animate.state.borrow_mut().canvas = canvas;
    }
}

impl Run for Renderer {
    async fn run(&mut self) -> Result<()> {
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
    canvas: Option<OffscreenCanvas>,
    queue: VecDeque<VideoFrame>,
    render: Option<Closure<dyn FnMut()>>,
}

impl RenderAnimate {
    pub fn new(canvas: Option<OffscreenCanvas>) -> Self {
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

        if let Some(ctx) = ctx.dyn_ref::<OffscreenCanvasRenderingContext2d>() {
            ctx.draw_image_with_video_frame(&frame, 0.0, 0.0).unwrap();
        } else if let Some(ctx) = ctx.dyn_ref::<CanvasRenderingContext2d>() {
            ctx.draw_image_with_video_frame(&frame, 0.0, 0.0).unwrap();
        } else {
            unreachable!("unsupported canvas context");
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
