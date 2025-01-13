use custom_elements::CustomElement;
use url::Url;
use wasm_bindgen::prelude::*;

use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlElement;

use super::{Backend, Controls, ControlsSend, State, Status, StatusRecv};
use crate::{Error, Result};

#[wasm_bindgen]
pub struct Element {
	controls: ControlsSend,
	status: StatusRecv,
}

#[wasm_bindgen]
impl Element {
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
		self.controls.url.as_ref().map(|u| u.to_string())
	}

	#[wasm_bindgen(setter)]
	pub fn set_url(&mut self, url: Option<String>) -> Result<()> {
		let url = url.map(|u| Url::parse(&u)).transpose().map_err(|_| Error::InvalidUrl)?;
		self.controls.url.send(url).map_err(|_| Error::Closed)
	}

	#[wasm_bindgen(getter)]
	pub fn paused(&self) -> bool {
		*self.controls.paused
	}

	#[wasm_bindgen(setter)]
	pub fn set_paused(&mut self, paused: bool) -> Result<()> {
		self.controls.paused.send(paused).map_err(|_| Error::Closed)
	}

	#[wasm_bindgen(getter)]
	pub fn volume(&self) -> f64 {
		*self.controls.volume
	}

	#[wasm_bindgen(setter)]
	pub fn set_volume(&mut self, volume: f64) -> Result<()> {
		self.controls.volume.send(volume).map_err(|_| Error::Closed)
	}

	#[wasm_bindgen(getter)]
	pub fn closed(&self) -> bool {
		*self.controls.close
	}

	#[wasm_bindgen(setter)]
	pub fn set_closed(&mut self, closed: bool) -> Result<()> {
		self.controls.close.send(closed).map_err(|_| Error::Closed)
	}

	pub async fn state(&mut self) -> State {
		self.status.state.recv().await.cloned().unwrap_or(State::Error)
	}
}

impl Default for Element {
	fn default() -> Self {
		Self::new()
	}
}

impl CustomElement for Element {
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
		&["name", "paused"]
	}

	fn attribute_changed_callback(
		&mut self,
		_this: &HtmlElement,
		name: String,
		_old_value: Option<String>,
		new_value: Option<String>,
	) {
		if name == "url" {
			self.set_url(new_value).ok();
		} else if name == "paused" {
			self.set_paused(new_value.is_some()).ok();
		}
	}

	fn connected_callback(&mut self, this: &HtmlElement) {
		let canvas = this
			.query_selector("slot[name=canvas]::slotted(canvas)")
			.unwrap()
			.expect("failed to find canvas")
			.unchecked_into();
		self.controls.canvas.send(Some(canvas)).ok();
	}

	fn disconnected_callback(&mut self, _this: &HtmlElement) {}

	fn adopted_callback(&mut self, _this: &HtmlElement) {}
}
