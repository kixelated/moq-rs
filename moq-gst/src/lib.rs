use gst::glib;

mod sink;

pub fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
	sink::register(plugin)?;
	Ok(())
}

gst::plugin_define!(
	moq,
	env!("CARGO_PKG_DESCRIPTION"),
	plugin_init,
	concat!(env!("CARGO_PKG_VERSION"), "-", env!("COMMIT_ID")),
	"MIT/Apache-2.0",
	env!("CARGO_PKG_NAME"),
	env!("CARGO_PKG_NAME"),
	env!("CARGO_PKG_REPOSITORY"),
	env!("BUILD_REL_DATE")
);
