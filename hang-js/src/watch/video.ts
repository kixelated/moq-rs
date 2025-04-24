import * as Moq from "@kixelated/moq";
import type * as Catalog from "../catalog";
import * as Container from "../container";
import type { Frame } from "../container/frame";
import { Context, Task } from "../util/context";
import * as Hex from "../util/hex";

export class Video {
	#render?: CanvasRenderingContext2D;

	#decoder: VideoDecoder;
	#visible = true;

	#connection?: Moq.Connection;
	#broadcast?: string;
	#tracks?: Catalog.Video[];

	#latency = 0;
	#running = new Task(this.#run.bind(this));
	#active?: Catalog.Video;

	constructor() {
		this.#decoder = new VideoDecoder({
			output: (frame) => this.#decoded(frame),
			error: (error) => console.error(error),
		});

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
		if (!this.#render || !this.#visible || !this.#tracks || !this.#connection || !this.#broadcast) {
			return await this.#running.abort();
		}

		const info = this.#tracks.at(0);
		if (!info) {
			return await this.#running.abort();
		}

		if (info === this.#active) {
			// No change; continue the current run.
			return;
		}

		this.#active = info;
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
			if (!this.#render) return;

			const width = frame.displayWidth ?? frame.codedWidth;
			const height = frame.displayHeight ?? frame.codedHeight;

			this.#render.canvas.width = width;
			this.#render.canvas.height = height;

			this.#render.drawImage(frame, 0, 0, width, height); // TODO respect aspect ratio
			frame.close();
		});
	}
}
