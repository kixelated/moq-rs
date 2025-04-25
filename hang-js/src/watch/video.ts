import * as Moq from "@kixelated/moq";
import type * as Catalog from "../catalog";
import * as Container from "../container";
import type { Frame } from "../container/frame";
import * as Hex from "../util/hex";

export class Video {
	#render?: CanvasRenderingContext2D;
	#track?: VideoTrack;

	#paused = false;
	#visible = true;

	// Save these variables so we can reload the video if `visible` or `canvas` changes.
	#connection?: Moq.Connection;
	#broadcast?: string;
	#tracks?: Catalog.Video[];

	constructor() {
		// TODO Implement IntersectionObserver too
		document.addEventListener("visibilitychange", () => {
			this.#visible = document.visibilityState === "visible";
			this.#reload();
		});
	}

	get canvas(): HTMLCanvasElement | undefined {
		return this.#render?.canvas;
	}

	set canvas(canvas: HTMLCanvasElement | undefined) {
		this.#render = canvas?.getContext("2d") ?? undefined;
	}

	get paused(): boolean {
		return this.#paused;
	}

	set paused(paused: boolean) {
		this.#paused = paused;
		this.#reload();
	}

	async catalog(connection: Moq.Connection, broadcast: string, tracks: Catalog.Video[]) {
		this.#connection = connection;
		this.#broadcast = broadcast;
		this.#tracks = tracks;

		this.#reload();
	}

	close() {
		this.#connection = undefined;
		this.#broadcast = undefined;
		this.#tracks = undefined;
		this.#track?.close();
	}

	#reload() {
		const existing = this.#track;
		this.#track = undefined;

		const info = this.#tracks?.at(0);

		if (
			!this.#render ||
			!this.#visible ||
			!this.#tracks ||
			!this.#connection ||
			!this.#broadcast ||
			!info ||
			this.#paused
		) {
			// Nothing to render, unsubscribe.
			existing?.close();
			return;
		}

		if (existing && info === existing.info) {
			// No change; continue the current run.
			this.#track = existing;
			return;
		}

		const track = this.#connection.subscribe(this.#broadcast, info.track.name, info.track.priority);
		this.#track = new VideoTrack(info, track, this.#render);
	}
}

export class VideoTrack {
	info: Catalog.Video;
	render: CanvasRenderingContext2D;

	#container: Container.Reader;
	#decoder: VideoDecoder;
	#queued?: VideoFrame;
	#animating?: number;

	constructor(info: Catalog.Video, track: Moq.TrackReader, render: CanvasRenderingContext2D) {
		this.info = info;
		this.render = render;

		this.#decoder = new VideoDecoder({
			output: (frame) => this.#decoded(frame),
			// TODO bubble up error
			error: (error) => this.#container.close(),
		});

		this.#decoder.configure({
			codec: info.codec,
			codedHeight: info.resolution.height,
			codedWidth: info.resolution.width,
			description: info.description ? Hex.decode(info.description) : undefined,
			optimizeForLatency: true,
		});

		this.#container = new Container.Reader(track);

		this.#run();
	}

	#decoded(frame: VideoFrame) {
		if (this.#animating) {
			this.#queued?.close();
			this.#queued = frame;
		} else {
			this.#animating = self.requestAnimationFrame(() => this.#draw(frame));
		}
	}

	#draw(frame: VideoFrame) {
		const width = frame.displayWidth ?? frame.codedWidth;
		const height = frame.displayHeight ?? frame.codedHeight;

		this.render.canvas.width = width;
		this.render.canvas.height = height;

		this.render.drawImage(frame, 0, 0, width, height); // TODO respect aspect ratio
		frame.close();

		if (this.#queued) {
			const frame = this.#queued;
			this.#queued = undefined;
			this.#animating = self.requestAnimationFrame(() => this.#draw(frame));
		} else {
			this.#animating = undefined;
		}
	}

	async #run() {
		for (;;) {
			const frame = await this.#container.readFrame();
			if (!frame) break;

			const chunk = new EncodedVideoChunk({
				type: frame.keyframe ? "key" : "delta",
				data: frame.data,
				timestamp: frame.timestamp,
			});

			this.#decoder.decode(chunk);
		}

		this.#decoder.close();
	}

	close() {
		this.#container.close();

		if (this.#animating) {
			self.cancelAnimationFrame(this.#animating);
			this.#animating = undefined;
		}

		this.#queued?.close();
	}
}
