import * as Moq from "@kixelated/moq";
import type * as Catalog from "../catalog";

import { Frame } from "../catalog/frame";
import * as Audio from "./audio";
import { Timeline } from "./timeline";
import * as Video from "./video";

// This class must be created on the main thread due to AudioContext.
export class Broadcast {
	#connection: Moq.Connection;
	#path: string;

	// Running is a promise that resolves when the player is closed.
	// #close is called with no error, while #abort is called with an error.
	#running: Promise<void>;

	// Timeline receives samples, buffering them and choosing the timestamp to render.
	#timeline = new Timeline();

	#audio?: Audio.Renderer;
	#video?: Video.Renderer;

	constructor(connection: Moq.Connection, path: string, catalog: Catalog.Broadcast, canvas: HTMLCanvasElement) {
		this.#connection = connection;
		this.#path = path;

		const running = [];

		// Only configure audio is we have an audio track
		const audio = (catalog.audio || []).at(0);
		if (audio) {
			this.#audio = new Audio.Renderer(audio, this.#timeline.audio);
			running.push(this.#runAudio(audio));
		}

		const video = (catalog.video || []).at(0);
		if (video) {
			this.#video = new Video.Renderer(video, canvas, this.#timeline.video);
			running.push(this.#runVideo(video));
		}

		// Async work
		this.#running = Promise.race([...running]);
	}

	async #runAudio(audio: Catalog.Audio) {
		const path = `${this.#path}/${audio.track.name}`;
		const track = new Moq.Track(path, audio.track.priority);
		const sub = await this.#connection.subscribe(track);

		try {
			for (;;) {
				const group = await Promise.race([sub.nextGroup(), this.#running]);
				if (!group) break;

				this.#runAudioGroup(audio, group)
					.catch(() => {})
					.finally(() => group.close());
			}
		} finally {
			sub.close();
		}
	}

	async #runVideo(video: Catalog.Video) {
		const path = `${this.#path}/${video.track.name}`;
		const track = new Moq.Track(path, video.track.priority);
		const sub = await this.#connection.subscribe(track);

		try {
			for (;;) {
				const group = await Promise.race([sub.nextGroup(), this.#running]);
				if (!group) break;

				this.#runVideoGroup(video, group)
					.catch(() => {})
					.finally(() => group.close());
			}
		} finally {
			sub.close();
		}
	}

	async #runAudioGroup(audio: Catalog.Audio, group: Moq.GroupReader) {
		const timeline = this.#timeline.audio;

		// Create a queue that will contain each frame
		const queue = new TransformStream<Frame>({});
		const segment = queue.writable.getWriter();

		// Add the segment to the timeline
		const segments = timeline.segments.getWriter();
		await segments.write({
			sequence: group.id,
			frames: queue.readable,
		});
		segments.releaseLock();

		// Read each chunk, decoding the MP4 frames and adding them to the queue.
		for (;;) {
			const frame = await Frame.decode(group);
			if (!frame) break;

			await segment.write(frame);
		}

		// We done.
		await segment.close();
	}

	async #runVideoGroup(video: Catalog.Video, group: Moq.GroupReader) {
		const timeline = this.#timeline.video;

		// Create a queue that will contain each MP4 frame.
		const queue = new TransformStream<Frame>({});
		const segment = queue.writable.getWriter();

		// Add the segment to the timeline
		const segments = timeline.segments.getWriter();
		await segments.write({
			sequence: group.id,
			frames: queue.readable,
		});
		segments.releaseLock();

		for (;;) {
			const frame = await Frame.decode(group);
			if (!frame) break;

			await segment.write(frame);
		}

		// We done.
		await segment.close();
	}

	unmute() {
		console.debug("unmuting audio");
		this.#audio?.play();
	}

	close() {
		this.#audio?.close();
		this.#video?.close();
	}
}
