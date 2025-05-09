import * as Media from "../media";
import * as Moq from "@kixelated/moq";
import { Buffer } from "buffer";
import { isEqual } from "lodash";

// Responsible for switching between video tracks and buffering frames.
export class VideoSource {
	broadcast: Moq.BroadcastReader;
	#tracks: Media.Video[] = [];

	#track?: VideoTrack;
	#frames: VideoFrame[] = [];

	// Constraints
	#enabled = false; // Must opt-in

	// The maximum latency in milliseconds.
	// The larger the value, the more tolerance we have for network jitter.
	// This should be at least 1 frame for smoother playback.
	#maxLatency = 20;

	// The difference between the wall clock units and the timestamp units, in seconds.
	#ref?: number;

	constructor(broadcast: Moq.BroadcastReader) {
		this.broadcast = broadcast;
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
		if (this.#enabled === enabled) {
			return;
		}

		this.#enabled = enabled;
		this.#reload();
	}

	// TODO add max bitrate/resolution/etc.

	#reload() {
		const info = this.#tracks.at(0);

		if (!this.#enabled || !info) {
			this.#reset();
			return;
		}

		if (isEqual(this.#track?.info, info)) {
			// No change
			return;
		}

		// We don't clear previous frames so we can seamlessly switch tracks.
		this.#track?.close();

		const sub = this.broadcast.subscribe(info.track.name, info.track.priority);
		this.#track = new VideoTrack(sub, info);

		this.#run(this.#track);
	}

	async #run(track: VideoTrack) {
		const frames = track.frames.getReader();

		for (;;) {
			const { value: frame } = await frames.read();
			if (!frame) break;

			this.#frames.push(frame);
			this.#prune();
		}

		frames.cancel();
	}

	// Returns the frame that should be rendered at the given timestamp.
	frame(nowMs: DOMHighResTimeStamp): VideoFrame | undefined {
		const now = nowMs * 1000;
		const maxLatency = this.#maxLatency * 1000;

		if (this.#frames.length === 0) {
			return;
		}

		// Advance the reference timestamp if we need to.
		// We want the newest frame (last) to be be scheduled at most maxLatency in the future.
		// NOTE: This never goes backwards, so clock drift will cause issues.
		const last = this.#frames[this.#frames.length - 1];
		if (!this.#ref || last.timestamp - this.#ref - now > maxLatency) {
			const newRef = last.timestamp - now - maxLatency;
			this.#ref = newRef;
		}

		const goal = this.#ref + now;

		// Find the frame that is the closest to the desired timestamp.
		for (let i = 1; i < this.#frames.length; i++) {
			const frame = this.#frames[i];
			const diff = frame.timestamp - goal;

			if (diff < 0) {
				// This frame was in the past; find one in the future.
				continue;
			}

			if (diff > goal - this.#frames[i - 1].timestamp) {
				// The previous frame is closer to the goal; use it instead.
				// This should smooth out the video a little bit.
				return this.#frames[i - 1];
			}

			return frame;
		}

		// Render the most recent frame if they're all scheduled in the past.
		return last;
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
		const maxLatency = this.#maxLatency * 1000;

		while (this.#frames.length > 1) {
			const first = this.#frames[0];
			const last = this.#frames[this.#frames.length - 1];

			// Compute the average duration of a frame, minus one so there's some wiggle room.
			const duration = (last.timestamp - first.timestamp) / (this.#frames.length - 1);

			if (last.timestamp - first.timestamp <= maxLatency + duration) {
				break;
			}

			const frame = this.#frames.shift();
			frame?.close();
		}
	}

	#reset() {
		this.#track?.close();
		this.#track = undefined;

		for (const frame of this.#frames) {
			frame.close();
		}

		this.#frames.length = 0;
	}

	close() {
		this.#reset();
		this.broadcast.close();
		this.#track?.close();
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
			output: (frame) => {
				this.#writer.write(frame);
			},
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
		this.#writer.close().catch(() => void 0);

		try {
			this.#decoder.close();
		} catch (_) {
			// ignore
		}
	}
}

// An component to render a video to a canvas.
export class VideoRenderer {
	#ctx?: CanvasRenderingContext2D;
	#source?: VideoSource;

	#animate?: number;
	#paused = false;

	get canvas(): HTMLCanvasElement | undefined {
		return this.#ctx?.canvas;
	}

	set canvas(canvas: HTMLCanvasElement | undefined) {
		this.#ctx = canvas?.getContext("2d") ?? undefined;
		this.#schedule();
	}

	get source(): VideoSource | undefined {
		return this.#source;
	}

	set source(source: VideoSource | undefined) {
		// Note: doesn't cleanup the previous source so it could be reused
		this.#source = source;
		this.#schedule();
	}

	// (re)schedule a render maybe.
	#schedule() {
		if (this.#ctx && !this.#paused && this.#source) {
			if (!this.#animate) {
				this.#animate = requestAnimationFrame(this.#render.bind(this));
			}
		} else {
			if (this.#animate) {
				cancelAnimationFrame(this.#animate);
				this.#animate = undefined;
			}
		}
	}

	#render(now: DOMHighResTimeStamp) {
		// Schedule the next render.
		this.#animate = undefined;
		this.#schedule();

		if (!this.#ctx) {
			throw new Error("scheduled without a canvas");
		}

		this.#ctx.save();
		this.#ctx.fillStyle = "#000";
		this.#ctx.clearRect(0, 0, this.#ctx.canvas.width, this.#ctx.canvas.height);

		const frame = this.#source?.frame(now);
		if (frame) {
			this.#ctx.canvas.width = frame.displayWidth;
			this.#ctx.canvas.height = frame.displayHeight;
			this.#ctx.drawImage(frame, 0, 0, this.#ctx.canvas.width, this.#ctx.canvas.height);
		}

		this.#ctx.restore();
	}

	// Close the track and all associated resources.
	close() {
		if (this.#animate) {
			cancelAnimationFrame(this.#animate);
			this.#animate = undefined;
		}
	}
}
