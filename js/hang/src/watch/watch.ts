import { Connection, ConnectionProps } from "../connection";
import { WatchAudio, WatchAudioProps } from "./audio";
import { WatchBroadcast, BroadcastProps } from "./broadcast";
import { WatchVideo, WatchVideoProps } from "./video";

export type WatchProps = {
	connection?: ConnectionProps;
	broadcast?: BroadcastProps;
	video?: WatchVideoProps;
	audio?: WatchAudioProps;
};

export class Watch {
	connection: Connection;
	broadcast: WatchBroadcast;
	video: WatchVideo;
	audio: WatchAudio;

	constructor(props?: WatchProps) {
		this.connection = new Connection(props?.connection);
		this.broadcast = new WatchBroadcast(this.connection, props?.broadcast);

		this.video = new WatchVideo(this.broadcast.video, props?.video);
		this.audio = new WatchAudio(this.broadcast.audio, props?.audio);
	}

	close() {
		this.connection.close();
		this.broadcast.close();
		this.audio.close();
		this.video.close();
	}

	static isSupported() {
		const quic = typeof WebTransport !== "undefined";
		const audio = typeof AudioDecoder !== "undefined";
		const video = typeof VideoDecoder !== "undefined";
		const streams = typeof WritableStream !== "undefined";

		const supported = {
			all: quic && audio && video && streams,
			quic,
			audio,
			video,
			streams,
		};

		return supported;
	}
}
