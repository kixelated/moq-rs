import * as Message from "./message"
import Audio from "./audio"
import Video from "./video"

export default class Renderer {
	audio: Audio
	video: Video

	constructor(config: Message.Config) {
		this.audio = new Audio(config)
		this.video = new Video(config)
	}

	push(frame: AudioData | VideoFrame) {
		if (isAudioData(frame)) {
			this.audio.push(frame)
		} else if (isVideoFrame(frame)) {
			this.video.push(frame)
		} else {
			throw new Error("unknown frame type")
		}
	}

	play(play: Message.Play) {
		this.audio.play(play)
		this.video.play(play)
	}
}

function isAudioData(frame: AudioData | VideoFrame): frame is AudioData {
	return frame instanceof AudioData
}

function isVideoFrame(frame: AudioData | VideoFrame): frame is VideoFrame {
	return frame instanceof VideoFrame
}
