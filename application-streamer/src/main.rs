use crate::xvfb_stream::FFmpegXvfbStream;
use crate::input_streamer::MoQInputStreamer;
use crate::video_file_stream::FFmpegVideoFileStream;
use crate::xvfb::Xvfb;
use crate::xvfb_user::XvfbUser;

mod input_streamer;
mod xvfb_stream;
mod xvfb;
mod xvfb_user;
mod video_file_stream;

const RESOLUTION: moq_karp::Dimensions = moq_karp::Dimensions { width: 1920, height: 1080 };
const PORT: u16 = 4443;
const FPS: u32 = 30;
const DISPLAY: u32 = 99;
const TEST_VIDEO_FILE_LOCATION: &str = "C:/Users/liamf/Documents/Bach3/Bachelorproef/application-streamer/dev/bbb.fmp4";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let mut xvfb = Xvfb::new(RESOLUTION, DISPLAY);
	let mut application = XvfbUser::new(&xvfb, "kate");
	let mut display_stream = FFmpegXvfbStream::new(FPS, &xvfb);
	// let mut video_stream = FFmpegVideoFileStream::new(TEST_VIDEO_FILE_LOCATION);

	xvfb.start();
	application.start();
	display_stream.start();
	// video_stream.start().await;

	let mut input_streamer = MoQInputStreamer::new(PORT, display_stream.stdout());
	// let mut input_streamer = MoQInputStreamer::new(PORT, video_stream.stdout());
	input_streamer.start().await;

	// video_stream.stop();
	// display_stream.stop();
	// application.stop().await;
	// xvfb.stop().await;

	Ok(())
}
