import { Buffer } from "buffer";
import * as Moq from "@kixelated/moq";
import * as Media from "../media";

import { deepEqual } from "assert";
import { isEqual } from "lodash";
import { Derived, Signal, Signals, signal } from "../signals";

export type VideoProps = {
	broadcast?: Moq.BroadcastReader;
	available?: Media.Video[];
	enabled?: boolean;
	latency?: number;
};

// Responsible for switching between video tracks and buffering frames.
export class Video {
	broadcast: Signal<Moq.BroadcastReader | undefined>;
	enabled: Signal<boolean>; // Don't download any longer

	// The maximum latency in milliseconds.
	// The larger the value, the more tolerance we have for network jitter.
	// This should be at least 1 frame for smoother playback.
	latency: Signal<number>;
	tracks: Signal<Media.Video[]>;

	selected: Derived<Media.Video | undefined>;

	#frames: VideoFrame[] = [];

	// The difference between the wall clock units and the timestamp units, in seconds.
	#ref?: number;

	#signals = new Signals();

	constructor(props?: VideoProps) {
		this.broadcast = signal(props?.broadcast);
		this.tracks = signal(props?.available ?? []);
		this.enabled = signal(props?.enabled ?? false);
		this.latency = signal(props?.latency ?? 50);

		this.selected = this.#signals.derived(() => this.tracks.get().at(0), {
			equals: (a, b) => isEqual(a, b),
		});
		this.#signals.effect(() => this.#init());

		// Update the ref when the latency changes.
		this.#signals.effect(() => {
			this.latency.get();
			this.#prune();
			this.#ref = undefined;
		});
	}

	#init() {
		const enabled = this.enabled.get();
		if (!enabled) return;

		const selected = this.selected.get();
		if (!selected) return;

		const broadcast = this.broadcast.get();
		if (!broadcast) return;

		// We don't clear previous frames so we can seamlessly switch tracks.
		const sub = broadcast.subscribe(selected.track.name, selected.track.priority);
		const media = new Media.Reader(sub);

		const decoder = new VideoDecoder({
			output: (frame) => {
				this.#frames.push(frame);
				this.#prune();
			},
			// TODO bubble up error
			error: (error) => {
				console.error(error);
				this.close();
			},
		});

		decoder.configure({
			codec: selected.codec,
			//codedHeight: info.resolution.height,
			//codedWidth: info.resolution.width,
			description: selected.description ? Buffer.from(selected.description, "hex") : undefined,
			optimizeForLatency: true,
		});

		(async () => {
			try {
				for (;;) {
					const frame = await media.readFrame();
					if (!frame) break;

					const chunk = new EncodedVideoChunk({
						type: frame.keyframe ? "key" : "delta",
						data: frame.data,
						timestamp: frame.timestamp,
					});

					decoder.decode(chunk);
				}
			} catch (error) {
				console.warn("video decoder error", error);
			} finally {
				media.close();
			}
		})();

		return () => {
			decoder.close();
			media.close();
			sub.close();
		};
	}

	// Returns the frame that should be rendered at the given timestamp.
	// Includes the latency offset to determine how timely the frame is.
	frame(nowMs: DOMHighResTimeStamp): { frame: VideoFrame; lag: DOMHighResTimeStamp } | undefined {
		const now = nowMs * 1000;
		const maxLatency = this.latency.peek() * 1000;

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
				return { frame: this.#frames[i - 1], lag: (this.#frames[i - 1].timestamp - goal) / 1000 };
			}

			return { frame, lag: diff / 1000 };
		}

		// Render the most recent frame if they're all scheduled in the past.
		return { frame: last, lag: (goal - last.timestamp) / 1000 };
	}

	#prune() {
		const maxLatency = this.latency.peek() * 1000;

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

	clear() {
		for (const frame of this.#frames) {
			frame.close();
		}

		this.#frames.length = 0;
	}

	close() {
		for (const frame of this.#frames) {
			frame.close();
		}

		this.#frames.length = 0;
		this.#signals.close();
	}
}

export type VideoRendererProps = {
	source: Video;
	canvas?: HTMLCanvasElement;
	paused?: boolean;
};

// An component to render a video to a canvas.
export class VideoRenderer {
	source: Video;

	canvas: Signal<HTMLCanvasElement | undefined>;
	paused: Signal<boolean>;

	#animate?: number;

	#ctx!: Derived<CanvasRenderingContext2D | undefined>;
	#signals = new Signals();

	constructor(props: VideoRendererProps) {
		this.source = props.source;
		this.canvas = signal(props.canvas);
		this.paused = signal(props.paused ?? false);

		this.#ctx = this.#signals.derived(() => this.canvas.get()?.getContext("2d") ?? undefined);
		this.#signals.effect(() => this.#schedule());
	}

	// (re)schedule a render maybe.
	#schedule() {
		if (this.#ctx.get() && !this.paused.get()) {
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

		const ctx = this.#ctx.get();
		if (!ctx) {
			throw new Error("scheduled without a canvas");
		}

		ctx.save();
		ctx.fillStyle = "#000";
		ctx.clearRect(0, 0, ctx.canvas.width, ctx.canvas.height);

		const closest = this.source.frame(now);
		if (closest) {
			ctx.canvas.width = closest.frame.displayWidth;
			ctx.canvas.height = closest.frame.displayHeight;
			ctx.drawImage(closest.frame, 0, 0, ctx.canvas.width, ctx.canvas.height);
		}

		ctx.restore();
	}

	// Close the track and all associated resources.
	close() {
		this.#signals.close();

		if (this.#animate) {
			cancelAnimationFrame(this.#animate);
			this.#animate = undefined;
		}
	}
}
