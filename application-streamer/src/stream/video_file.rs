use crate::FFmpegStream;

pub fn new(file_location: &str) -> FFmpegStream {
    FFmpegStream::new(vec![
        "-hide_banner",
        "-v", "quiet",
        "-stream_loop", "-1",
        "-re",
        "-i", file_location,
        "-c", "copy",
        "-f", "mp4",
        "-movflags", "cmaf+separate_moof+delay_moov+skip_trailer+frag_every_frame",
        "pipe:1"
    ])
}