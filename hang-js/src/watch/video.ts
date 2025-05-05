import * as Moq from "@kixelated/moq";
import * as Media from "../media";

// biome-ignore lint/style/useNodejsImportProtocol: browser polyfill
import { Buffer } from "buffer";

// Responsible for choosing the best video track for an active broadcast.
export class VideoTracks {
	broadcast: Moq.BroadcastReader;
	#tracks: Media.Video[] = [];
	#active?: VideoTrack;

	// constraints
	#enabled = true;

	// The combined video frames from all tracks.
	frames: ReadableStream<VideoFrame>;
	#writer: WritableStream<VideoFrame>;

	constructor(broadcast: Moq.BroadcastReader) {
		this.broadcast = broadcast;

		const queue = new TransformStream<VideoFrame, VideoFrame>();
		this.#writer = queue.writable;
		this.frames = queue.readable;
	}

	get tracks(): Media.Video[] {
		return this.#tracks;
	}

	set tracks(tracks: Media.Video[]) {
		this.#tracks = tracks;
		this.#reload();
	}

	get enabled(): boolean {
		return this.#enabled;
	}

	set enabled(enabled: boolean) {
		this.#enabled = enabled;
		this.#reload();
	}

	// TODO add max bitrate/resolution/etc.

	#reload() {
		if (!this.#enabled) {
			this.#active?.close();
			this.#active = undefined;
			return;
		}

		const info = this.#tracks.at(0);
		if (this.#active?.info === info) {
			return;
		}

		this.#active?.close();
		this.#active = undefined;

		if (info) {
			const track = this.broadcast.subscribe(info.track.name, info.track.priority);
			this.#active = new VideoTrack(track, info);
			this.#active.frames.pipeTo(this.#writer, { preventClose: true, preventCancel: true });
		}
	}

	close() {
		this.broadcast.close();
		this.#active?.close();
	}
}

// Responsible for decoding video frames for the specified track.
export class VideoTrack {
	// The video track info.
	info: Media.Video;

	// The decoded frames.
	frames: ReadableStream<VideoFrame>;

	#writer: WritableStreamDefaultWriter<VideoFrame>;

	#media: Media.Reader;

	#decoder: VideoDecoder;

	constructor(track: Moq.TrackReader, info: Media.Video) {
		this.info = info;

		const queue = new TransformStream<VideoFrame, VideoFrame>();
		this.frames = queue.readable;
		this.#writer = queue.writable.getWriter();

		this.#decoder = new VideoDecoder({
			output: (frame) => this.#writer.write(frame),
			// TODO bubble up error
			error: (error) => {
				console.error(error);
				this.close();
			},
		});

		this.#decoder.configure({
			codec: info.codec,
			//codedHeight: info.resolution.height,
			//codedWidth: info.resolution.width,
			description: info.description ? Buffer.from(info.description, "hex") : undefined,
			optimizeForLatency: true,
		});

		this.#media = new Media.Reader(track);

		this.#run().finally(() => this.close());
	}

	async #run() {
		for (;;) {
			const frame = await this.#media.readFrame();
			if (!frame) break;

			const chunk = new EncodedVideoChunk({
				type: frame.keyframe ? "key" : "delta",
				data: frame.data,
				timestamp: frame.timestamp,
			});

			this.#decoder.decode(chunk);
		}
	}

	// Close the track and all associated resources.
	close() {
		this.#media.close();
		this.#writer.close();

		try {
			this.#decoder.close();
		} catch (_) {
			// ignore
		}
	}
}

// An optional component to render a video to a canvas.
export class VideoRenderer {
	#ctx?: CanvasRenderingContext2D;
	#paused = false;
	#broadcast?: VideoTracks;

	#animate?: number;
	#frames: VideoFrame[] = [];

	// The difference between the wall clock units and the timestamp units, in seconds.
	#ref?: number;

	// The maximum latency in milliseconds.
	// The larger the value, the more tolerance we have for network jitter.
	// This should be at least 1 frame for smoother playback.
	#maxLatency = 20;

	#writer: WritableStream<VideoFrame>;
	#reader: ReadableStreamDefaultReader<VideoFrame>;

	constructor() {
		const queue = new TransformStream<VideoFrame, VideoFrame>();
		this.#writer = queue.writable;
		this.#reader = queue.readable.getReader();

		this.#run().finally(() => this.close());
	}

	get canvas(): HTMLCanvasElement | undefined {
		return this.#ctx?.canvas;
	}

	set canvas(canvas: HTMLCanvasElement | undefined) {
		this.#ctx = canvas?.getContext("2d") ?? undefined;
		this.#reload();
	}

	get paused(): boolean {
		return this.#paused;
	}

	set paused(paused: boolean) {
		this.#paused = paused;
		this.#reload();
	}

	get broadcast(): VideoTracks | undefined {
		return this.#broadcast;
	}

	set broadcast(broadcast: VideoTracks | undefined) {
		this.#broadcast?.close();

		this.#broadcast = broadcast;
		broadcast?.frames.pipeTo(this.#writer, { preventClose: true, preventCancel: true });

		this.#reload();
	}

	#reload() {
		if (this.#animate) {
			cancelAnimationFrame(this.#animate);
		}

		if (!this.#ctx || !this.#broadcast || this.#paused) {
			if (this.#broadcast) {
				this.#broadcast.enabled = false;
			}

			if (this.#animate) {
				cancelAnimationFrame(this.#animate);
			}

			return;
		}

		this.#broadcast.enabled = true;
		this.#animate = requestAnimationFrame(this.#render.bind(this));
	}

	async #run() {
		for (;;) {
			const { done, value } = await this.#reader.read();
			if (done) break;

			this.#frames.push(value);
			this.#prune();
		}
	}

	// Returns the frame that should be rendered at the given timestamp.
	#frame(nowMs: DOMHighResTimeStamp): VideoFrame | undefined {
		const now = nowMs * 1000;

		if (this.#frames.length === 0) {
			return;
		}

		const last = this.#frames[this.#frames.length - 1];

		if (!this.#ref || now - last.timestamp > this.#ref) {
			this.#ref = now - last.timestamp;
		}

		// Find the frame that is the closest to the desired timestamp.
		const goal = now - this.#ref;

		for (let i = 0; i < this.#frames.length; i++) {
			let frame = this.#frames[i];
			const diff = frame.timestamp - goal;

			if (diff > 0) {
				// We want to render in the future.
				continue;
			}

			// Check if the previous frame was closer (we continued; over it)
			if (i > 0 && frame.timestamp - goal < goal - this.#frames[i - 1].timestamp) {
				frame = this.#frames[i - 1];
			}

			return frame;
		}

		// Render the most recent frame.
		return last;
	}

	#render(now: DOMHighResTimeStamp) {
		if (!this.#ctx || !this.#broadcast) {
			return;
		}

		this.#animate = requestAnimationFrame(this.#render.bind(this));

		this.#ctx.save();
		this.#ctx.clearRect(0, 0, this.#ctx.canvas.width, this.#ctx.canvas.height);
		this.#ctx.font = "12px sans-serif";
		this.#ctx.fillStyle = "#000";

		const frame = this.#frame(now);
		if (!frame) {
			return;
		}

		this.#ctx.canvas.width = frame.displayWidth;
		this.#ctx.canvas.height = frame.displayHeight;

		this.#ctx.drawImage(frame, 0, 0, this.#ctx.canvas.width, this.#ctx.canvas.height);
	}

	get latency(): DOMHighResTimeStamp {
		return this.#maxLatency;
	}

	set latency(latency: DOMHighResTimeStamp) {
		this.#maxLatency = latency;
		this.#ref = undefined;
		this.#prune();
	}

	#prune() {
		while (
			this.#frames.length > 1 &&
			this.#frames[this.#frames.length - 1].timestamp - this.#frames[0].timestamp > this.#maxLatency * 1000
		) {
			const frame = this.#frames.shift();
			frame?.close();
		}
	}

	// Close the track and all associated resources.
	close() {
		for (const frame of this.#frames) {
			frame.close();
		}

		this.#frames.length = 0;

		this.#reader.cancel().catch(() => void 0);
		this.#writer.close().catch(() => void 0);
	}
}
