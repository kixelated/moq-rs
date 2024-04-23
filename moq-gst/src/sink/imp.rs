use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_base::subclass::prelude::*;

use once_cell::sync::Lazy;
use std::sync::Mutex;

#[derive(Default)]
struct Settings {
	pub url: Option<String>,
	pub namespace: Option<String>,
}

#[derive(Default)]
pub struct MoqSink {
	settings: Mutex<Settings>,
	//state: Mutex<State>,
}

#[glib::object_subclass]
impl ObjectSubclass for MoqSink {
	const NAME: &'static str = "MoqSink";
	type Type = super::MoqSink;
	type ParentType = gst_base::BaseSink;

	fn new() -> Self {
		Self::default()
	}
}

impl ObjectImpl for MoqSink {
	fn properties() -> &'static [glib::ParamSpec] {
		static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
			vec![
				glib::ParamSpecString::builder("url")
					.nick("URL")
					.blurb("Connect to the subscriber at the given URL")
					.build(),
				glib::ParamSpecString::builder("namespace")
					.nick("Namespace")
					.blurb("Publish the broadcast under the given namespace")
					.build(),
			]
		});
		PROPERTIES.as_ref()
	}

	fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
		let mut settings = self.settings.lock().unwrap();

		match pspec.name() {
			"url" => settings.url = Some(value.get().unwrap()),
			"namespace" => settings.namespace = Some(value.get().unwrap()),
			_ => unimplemented!(),
		}
	}

	fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
		let settings = self.settings.lock().unwrap();

		match pspec.name() {
			"url" => settings.url.to_value(),
			"namespace" => settings.namespace.to_value(),
			_ => unimplemented!(),
		}
	}
}

impl GstObjectImpl for MoqSink {}

impl ElementImpl for MoqSink {
	fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
		static ELEMENT_METADATA: Lazy<gst::subclass::ElementMetadata> = Lazy::new(|| {
			gst::subclass::ElementMetadata::new(
				"MoQ Sink",
				"Sink",
				"Transmits media over the network via MoQ",
				"Luke Curley <kixelated@gmail.com>",
			)
		});

		Some(&*ELEMENT_METADATA)
	}

	fn pad_templates() -> &'static [gst::PadTemplate] {
		static PAD_TEMPLATES: Lazy<Vec<gst::PadTemplate>> = Lazy::new(|| {
			let caps = gst::Caps::builder("video/quicktime")
				.field("variant", "iso-fragmented")
				.build();

			let pad_template = gst::PadTemplate::with_gtype(
				"sink_%u",
				gst::PadDirection::Sink,
				gst::PadPresence::Request,
				&caps,
				super::MoqSink::static_type(),
			)
			.unwrap();

			vec![pad_template]
		});
		PAD_TEMPLATES.as_ref()
	}

	fn request_new_pad(
		&self,
		templ: &gst::PadTemplate,
		name: Option<&str>,
		caps: Option<&gst::Caps>,
	) -> Option<gst::Pad> {
		println!("Requesting new pad with name: {:?} and caps: {:?}", name, caps);
		let pad = self.parent_request_new_pad(templ, name, caps);
		if let Some(ref pad) = pad {
			println!("Pad successfully created: {:?}", pad.name());
		// Additional setup or initialization for the pad can go here
		} else {
			println!("Failed to create pad");
		}
		pad
	}
}

impl BaseSinkImpl for MoqSink {
	fn start(&self) -> Result<(), gst::ErrorMessage> {
		let settings = self.settings.lock().unwrap();

		/*
		let session = web_transport_quinn::connect(client, settings.url.expect("missing url"));

		moq_transport::Publisher::connect(&settings.url)
			.map_err(|err| gst::error_msg!(gst::CoreError::Failed, ("Failed to connect: {}", err)))?;

		// Example: Initialize a TCP connection
		let stream = TcpStream::connect(&settings.url)
			.map_err(|err| gst::error_msg!(gst::CoreError::Failed, ("Failed to connect: {}", err)))?;

		// Store the stream in your struct for later use
		self.stream.lock().unwrap().replace(stream);
		*/

		Ok(())
	}

	fn stop(&self) -> Result<(), gst::ErrorMessage> {
		Ok(())
	}

	fn render(&self, buffer: &gst::Buffer) -> Result<gst::FlowSuccess, gst::FlowError> {
		/*
		let caps = buffer.caps().ok_or(gst::FlowError::Error)?;

		if caps.is_equal(self.mp4_caps()) {
			// Process MP4 data here
			println!("Received MP4 buffer");
		// Additional handling logic
		} else {
			return Err(gst::FlowError::NotNegotiated);
		}
		*/

		let data = buffer.map_readable().map_err(|_| gst::FlowError::Error)?;

		// Here you would typically handle the data, e.g., sending it over the network.
		// This example simply prints the size of the incoming buffer.
		println!("Received buffer of size {}", data.size());

		// Insert network sending logic here.
		// For example, if using a TCP socket:
		// self.send_over_network(data.as_slice());

		Ok(gst::FlowSuccess::Ok)
	}
}
