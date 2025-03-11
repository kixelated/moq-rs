use application_streamer::{stream, MoQInputStreamer};

const RESOLUTION: moq_karp::Dimensions = moq_karp::Dimensions { width: 1920, height: 1080 };
const PORT: u16 = 4443;
const FPS: u32 = 30;
const DISPLAY: u32 = 99;
const TEST_VIDEO_FILE_LOCATION: &str = "C:/Users/liamf/Documents/Bach3/Bachelorproef/application-streamer/dev/bbb.fmp4";

/// Stream video file with moq-karp
#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let mut video_stream = stream::video_file::new(TEST_VIDEO_FILE_LOCATION);

	video_stream.start().await;

	let mut input_streamer = MoQInputStreamer::new(PORT);
	input_streamer.stream(video_stream.stdout()).await?; // blocking method

	video_stream.stop().await;

	Ok(())
}

// ///Stream xvfb with moq-karp
// #[tokio::main]
// async fn main() -> anyhow::Result<()> {
// 	let mut xvfb = Xvfb::new(RESOLUTION, DISPLAY);
// 	let mut application = XvfbUser::new(&xvfb, "kate");
// 	let mut display_stream = FFmpegXvfbStream::new(FPS, &xvfb);
//
// 	xvfb.start();
// 	application.start();
// 	display_stream.start();
//
// 	let mut input_streamer = MoQInputStreamer::new(PORT, display_stream.stdout());
// 	input_streamer.stream(display_stream.stdout()).await?; // blocking method
//
// 	display_stream.stop().await;
// 	application.stop().await;
// 	xvfb.stop().await;
//
// 	Ok(())
// }