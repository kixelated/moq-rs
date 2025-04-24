import * as Moq from "@kixelated/moq";
import type * as Catalog from "../catalog";
import * as Container from "../container";
import type { Frame } from "../container/frame";
import { Context, Task } from "../util/context";
import * as Hex from "../util/hex";

export class Video {
	#canvas?: HTMLCanvasElement;
	#decoder: VideoDecoder;
	#visible = true;

	#connection?: Moq.Connection;
	#broadcast?: string;
	#tracks?: Catalog.Video[];

	#latency = 0;
	#running = new Task(this.#run);

	#observer: IntersectionObserver;

	constructor() {
		this.#decoder = new VideoDecoder({
			output: (frame) => this.#decoded(frame),
			error: (error) => console.error(error),
		});

		// TODO use MutationObserver to detect `display: none`?
		this.#observer = new IntersectionObserver(
			([canvas]) => {
				this.#visible = canvas.isIntersecting;
				this.#reload();
			},
			{ threshold: 0 },
		);
	}

	get canvas(): HTMLCanvasElement | undefined {
		return this.#canvas;
	}

	set canvas(canvas: HTMLCanvasElement | undefined) {
		if (this.#canvas) {
			this.#observer.unobserve(this.#canvas);
		}

		this.#canvas = canvas;
		if (canvas) {
			this.#observer.observe(canvas);
		}
	}

	set latency(latency: number) {
		this.#latency = latency;
	}

	async load(connection: Moq.Connection, broadcast: string, tracks: Catalog.Video[]) {
		this.#connection = connection;
		this.#broadcast = broadcast;
		this.#tracks = tracks;

		await this.#reload();
	}

	async abort() {
		this.#connection = undefined;
		this.#broadcast = undefined;
		this.#tracks = undefined;

		await this.#running.abort();
	}

	async #reload() {
		await this.#running.abort();
		console.log(this);

		if (!this.#canvas) return;
		if (!this.#visible) return;
		if (!this.#tracks || !this.#connection || !this.#broadcast) return;

		const info = this.#tracks.at(0);
		if (!info) throw new Error("no video track");

		console.log("video track:", info);

		this.#running.start(this.#connection, this.#broadcast, info);
	}

	async #run(context: Context, connection: Moq.Connection, broadcast: string, info: Catalog.Video) {
		this.#decoder.configure({
			codec: info.codec,
			codedHeight: info.resolution.height,
			codedWidth: info.resolution.width,
			description: info.description ? Hex.decode(info.description) : undefined,
			optimizeForLatency: true,
		});

		const track = connection.subscribe(broadcast, info.track.name, info.track.priority);
		const container = new Container.Reader(track);

		try {
			for (;;) {
				const frame = await context.race(container.readFrame());
				if (!frame) throw new Error("no frame");

				const chunk = new EncodedVideoChunk({
					type: frame.keyframe ? "key" : "delta",
					data: frame.data,
					timestamp: frame.timestamp,
				});

				this.#decoder.decode(chunk);
			}
		} finally {
			container.close();
		}
	}

	#decoded(frame: VideoFrame) {
		self.requestAnimationFrame(() => {
			if (!this.#canvas) return;

			this.#canvas.width = frame.displayWidth;
			this.#canvas.height = frame.displayHeight;

			const ctx = this.#canvas.getContext("2d");
			if (!ctx) throw new Error("failed to get canvas context");

			ctx.drawImage(frame, 0, 0, frame.displayWidth, frame.displayHeight); // TODO respect aspect ratio
			frame.close();
		});
	}
}
