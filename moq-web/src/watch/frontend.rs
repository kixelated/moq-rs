use custom_elements::CustomElement;
use url::Url;
use wasm_bindgen::prelude::*;

use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlCanvasElement, HtmlElement, HtmlSlotElement};

use super::{Backend, Controls, ControlsSend, State, Status, StatusRecv};
use crate::{Error, Result};

#[wasm_bindgen]
pub struct Watch {
	controls: ControlsSend,
	status: StatusRecv,
}

#[wasm_bindgen]
impl Watch {
	#[wasm_bindgen(constructor)]
	pub fn new() -> Self {
		let controls = Controls::default().baton();
		let status = Status::default().baton();

		let mut backend = Backend::new(controls.1, status.0);
		spawn_local(async move {
			if let Err(err) = backend.run().await {
				tracing::error!(?err, "backend error");
			}
		});

		Self {
			controls: controls.0,
			status: status.1,
		}
	}

	#[wasm_bindgen(getter)]
	pub fn url(&self) -> Option<String> {
		self.controls.url.get().map(|u| u.to_string())
	}

	#[wasm_bindgen(setter)]
	pub fn set_url(&mut self, url: Option<String>) -> Result<()> {
		let url = url.map(|u| Url::parse(&u)).transpose().map_err(|_| Error::InvalidUrl)?;
		self.controls.url.send(url).map_err(|_| Error::Closed)
	}

	#[wasm_bindgen(getter)]
	pub fn paused(&self) -> bool {
		self.controls.paused.get()
	}

	#[wasm_bindgen(setter)]
	pub fn set_paused(&mut self, paused: bool) -> Result<()> {
		self.controls.paused.send(paused).map_err(|_| Error::Closed)
	}

	#[wasm_bindgen(getter)]
	pub fn volume(&self) -> f64 {
		self.controls.volume.get()
	}

	#[wasm_bindgen(setter)]
	pub fn set_volume(&mut self, volume: f64) -> Result<()> {
		self.controls.volume.send(volume).map_err(|_| Error::Closed)
	}

	#[wasm_bindgen(getter)]
	pub fn closed(&self) -> bool {
		self.controls.close.get()
	}

	#[wasm_bindgen(setter)]
	pub fn set_closed(&mut self, closed: bool) -> Result<()> {
		self.controls.close.send(closed).map_err(|_| Error::Closed)
	}

	pub async fn state(&mut self) -> State {
		self.status.state.recv().await.unwrap_or(State::Error)
	}
}

impl Default for Watch {
	fn default() -> Self {
		Self::new()
	}
}

impl CustomElement for Watch {
	fn inject_children(&mut self, this: &HtmlElement) {
		let document = web_sys::window().unwrap().document().unwrap();

		let style = document.create_element("style").unwrap();
		let canvas_slot = document.create_element("slot").unwrap();
		let canvas_default = document.create_element("canvas").unwrap();

		canvas_slot.set_attribute("name", "canvas").unwrap();
		canvas_slot.append_child(&canvas_default).unwrap();
		this.append_child(&canvas_slot).unwrap();

		style.set_text_content(Some(
			"
			:host {
				display: block;
				position: relative;
			}

			canvas {
				display: block;
				max-width: 100%;
				height: auto;
			}
		",
		));

		this.append_child(&style).unwrap();
	}

	fn observed_attributes() -> &'static [&'static str] {
		&["url", "paused"]
	}

	fn attribute_changed_callback(
		&mut self,
		_this: &HtmlElement,
		name: String,
		old_value: Option<String>,
		new_value: Option<String>,
	) {
		tracing::info!(?name, ?old_value, ?new_value, "attribute changed");
		if old_value == new_value {
			return;
		}

		if name == "url" {
			self.set_url(new_value).ok();
		} else if name == "paused" {
			self.set_paused(new_value.is_some()).ok();
		}
	}

	fn connected_callback(&mut self, this: &HtmlElement) {
		let canvas = find_canvas(this);
		gloo_console::log!("canvas", &canvas);

		self.controls.canvas.send(Some(canvas)).ok();

		self.attribute_changed_callback(this, "url".to_string(), None, this.get_attribute("url"));
		self.attribute_changed_callback(this, "paused".to_string(), None, this.get_attribute("paused"));
	}

	fn disconnected_callback(&mut self, _this: &HtmlElement) {}

	fn adopted_callback(&mut self, _this: &HtmlElement) {}
}

// Find the <slot name="canvas"> element and extract the <canvas> element from it.
fn find_canvas(element: &HtmlElement) -> HtmlCanvasElement {
	let slot: HtmlSlotElement = element
		.shadow_root()
		.unwrap()
		.query_selector("slot[name=canvas]")
		.unwrap()
		.expect("failed to find canvas slot")
		.unchecked_into();

	// We flatten the assigned nodes to handle nested slots.
	let options = web_sys::AssignedNodesOptions::new();
	options.set_flatten(true);

	for node in slot.assigned_nodes_with_options(&options) {
		// If it's a <canvas>, return it.
		if let Some(nested) = node.dyn_ref::<HtmlCanvasElement>() {
			return nested.clone();
		} else if let Some(parent) = node.dyn_ref::<HtmlElement>() {
			// If it's a <div> or other element, search its children.
			return parent
				.query_selector("canvas")
				.unwrap()
				.expect("failed to find canvas")
				.unchecked_into();
		}
	}

	panic!("failed to find canvas")
}
