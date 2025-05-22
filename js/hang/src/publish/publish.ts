import { Signal, Signals, signal } from "@kixelated/signals";
import { Connection, ConnectionProps } from "../connection";
import { Broadcast, BroadcastProps } from "./broadcast";

export interface PublishProps {
	connection?: ConnectionProps;
	broadcast?: BroadcastProps;
	preview?: HTMLVideoElement;
}

export class Publish {
	connection: Connection;
	broadcast: Broadcast;
	preview: Signal<HTMLVideoElement | undefined>;

	#signals = new Signals();

	constructor(props?: PublishProps) {
		this.connection = new Connection(props?.connection);
		this.broadcast = new Broadcast(this.connection, props?.broadcast);
		this.preview = signal<HTMLVideoElement | undefined>(props?.preview);

		this.#signals.effect(() => {
			const media = this.broadcast.video.media.get();
			const preview = this.preview.get();
			if (!preview || !media) return;

			preview.srcObject = new MediaStream([media]) ?? null;
			return () => {
				preview.srcObject = null;
			};
		});

		// Only publish when we have media available.
		this.#signals.effect(() => {
			const audio = this.broadcast.audio.media.get();
			const video = this.broadcast.video.media.get();
			this.broadcast.publish.set(!!audio || !!video);
		});
	}

	close() {
		this.#signals.close();
		this.broadcast.close();
		this.connection.close();
	}
}
