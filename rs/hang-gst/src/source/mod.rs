use gst::glib;
use gst::prelude::*;

mod imp;

glib::wrapper! {
	pub struct HangSrc(ObjectSubclass<imp::HangSrc>) @extends gst::Bin, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
	gst::Element::register(Some(plugin), "hangsrc", gst::Rank::NONE, HangSrc::static_type())
}
