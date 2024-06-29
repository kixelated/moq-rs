/// Use sane defaults for logging
pub fn init() {
	// Use the RUST_LOG environment variable if set, otherwise default to info
	let filter = tracing_subscriber::EnvFilter::try_from_default_env()
		.unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

	let logger = tracing_subscriber::FmtSubscriber::builder()
		.with_env_filter(filter)
		.finish();

	tracing::subscriber::set_global_default(logger).unwrap();
}
