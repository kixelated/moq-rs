use crate::xvfb_stream::FFmpegXvfbStream;
use crate::input_streamer::MoQInputStreamer;
use crate::xvfb::Xvfb;
use crate::xvfb_user::XvfbUser;

mod input_streamer;
mod xvfb_stream;
mod xvfb;
mod xvfb_user;

const RESOLUTION: moq_karp::Dimensions = moq_karp::Dimensions { width: 1920, height: 1080 };
const PORT: u16 = 4443;
const FPS: u32 = 30;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let mut xvfb = Xvfb::new(RESOLUTION, 99);
	let mut program = XvfbUser::new(&xvfb, "kate");
	let mut display_stream = FFmpegXvfbStream::new(FPS, &xvfb);

	xvfb.start().await;
	program.start().await;
	display_stream.start().await;

	let mut input_streamer = MoQInputStreamer::new(PORT, display_stream.stdout());
	input_streamer.start().await;

	display_stream.stop();
	program.stop().await;
	xvfb.stop().await;

	Ok(())
}
