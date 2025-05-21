use gst::glib;
use gst::prelude::*;

mod imp;

glib::wrapper! {
	pub struct HangSink(ObjectSubclass<imp::HangSink>) @extends gst_base::BaseSink, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
	gst::Element::register(Some(plugin), "hangsink", gst::Rank::NONE, HangSink::static_type())
}
