import * as Moq from "@kixelated/moq";
import type * as Catalog from "../catalog";
import * as Container from "../container";
import * as Hex from "../util/hex";

// 50ms in microseconds.
const MAX_LATENCY = 50000;

export class Video {
	#render?: CanvasRenderingContext2D;
	#track?: VideoTrack;

	#paused = false;
	#visible = true;

	// Save these variables so we can reload the video if `visible` or `canvas` changes.
	#broadcast?: Moq.BroadcastReader;
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

	load(broadcast: Moq.BroadcastReader, tracks: Catalog.Video[]) {
		this.#broadcast?.close();
		this.#broadcast = broadcast;
		this.#tracks = tracks;

		this.#reload();
	}

	close() {
		this.#broadcast?.close();
		this.#broadcast = undefined;
		this.#tracks = undefined;
		this.#track?.close();
	}

	#reload() {
		const existing = this.#track;
		this.#track = undefined;

		const info = this.#tracks?.at(0);

		if (!this.#render || !this.#visible || !this.#tracks || !this.#broadcast || !info || this.#paused) {
			// Nothing to render, unsubscribe.
			existing?.close();
			return;
		}

		if (existing && info === existing.info) {
			// No change; continue the current run.
			this.#track = existing;
			return;
		}

		const track = this.#broadcast.subscribe(info.track.name, info.track.priority);
		this.#track = new VideoTrack(info, track, this.#render);
	}
}

export class VideoTrack {
	info: Catalog.Video;
	render: CanvasRenderingContext2D;

	#container: Container.Reader;
	#decoder: VideoDecoder;
	#queued: VideoFrame[] = [];
	#animating?: number;

	constructor(info: Catalog.Video, track: Moq.TrackReader, render: CanvasRenderingContext2D) {
		this.info = info;
		this.render = render;

		this.#decoder = new VideoDecoder({
			output: (frame) => this.#decoded(frame as VideoFrame),
			// TODO bubble up error
			error: (error) => {
				console.error(error);
				this.close();
			},
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
		this.#queued.push(frame);
		this.#purge();

		if (!this.#animating) {
			this.#animating = self.requestAnimationFrame(() => this.#draw());
		}
	}

	#draw() {
		const frame = this.#queued.shift();
		if (!frame) return;

		const width = frame.displayWidth ?? frame.codedWidth;
		const height = frame.displayHeight ?? frame.codedHeight;

		this.render.canvas.width = width;
		this.render.canvas.height = height;

		this.render.drawImage(frame, 0, 0, width, height); // TODO respect aspect ratio
		frame.close();

		if (this.#queued.length > 0) {
			this.#animating = self.requestAnimationFrame(() => this.#draw());
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

		for (const frame of this.#queued) {
			frame.close();
		}

		this.#queued = [];
	}

	// Drop frames in the queue that are too old.
	#purge() {
		while (this.#queued.length > 1) {
			const first = this.#queued[0];
			const last = this.#queued[this.#queued.length - 1];

			if (first.timestamp + MAX_LATENCY > last.timestamp) {
				break;
			}

			this.#queued.shift();
			first.close();
		}
	}
}
