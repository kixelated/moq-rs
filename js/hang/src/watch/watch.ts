import { Connection, ConnectionProps } from "../connection";
import { Audio, AudioProps } from "./audio";
import { Broadcast, BroadcastProps } from "./broadcast";
import { Video, VideoProps } from "./video";

export type WatchProps = {
	connection?: ConnectionProps;
	broadcast?: BroadcastProps;
	video?: VideoProps;
	audio?: AudioProps;
};

export class Watch {
	connection: Connection;
	broadcast: Broadcast;
	video: Video;
	audio: Audio;

	constructor(props?: WatchProps) {
		this.connection = new Connection(props?.connection);
		this.broadcast = new Broadcast(this.connection, props?.broadcast);

		this.video = new Video(this.broadcast.video, props?.video);
		this.audio = new Audio(this.broadcast.audio, props?.audio);
	}

	close() {
		this.connection.close();
		this.broadcast.close();
		this.audio.close();
		this.video.close();
	}
}
