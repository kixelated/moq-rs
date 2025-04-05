import type * as Catalog from "../catalog";
import type { Frame } from "../container/frame";
import * as Hex from "../util/hex";

export class Video {
	#canvas: HTMLCanvasElement;
	#decoder: VideoDecoder;

	constructor(track: Catalog.Video, canvas: HTMLCanvasElement) {
		this.#canvas = canvas;
		this.#decoder = new VideoDecoder({
			output: (frame) => this.#decoded(frame),
			error: (error) => console.error(error),
		});

		this.#decoder.configure({
			codec: track.codec,
			codedHeight: track.resolution.height,
			codedWidth: track.resolution.width,
			description: track.description ? Hex.decode(track.description) : undefined,
			optimizeForLatency: true,
		});
	}

	decode(frame: Frame) {
		const chunk = new EncodedVideoChunk({
			type: frame.keyframe ? "key" : "delta",
			data: frame.data,
			timestamp: frame.timestamp,
		});

		this.#decoder.decode(chunk);
	}

	#decoded(frame: VideoFrame) {
		self.requestAnimationFrame(() => {
			this.#canvas.width = frame.displayWidth;
			this.#canvas.height = frame.displayHeight;

			const ctx = this.#canvas.getContext("2d");
			if (!ctx) throw new Error("failed to get canvas context");

			ctx.drawImage(frame, 0, 0, frame.displayWidth, frame.displayHeight); // TODO respect aspect ratio
			frame.close();
		});
	}
}
