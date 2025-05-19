import { Buffer } from "buffer";
import * as Moq from "@kixelated/moq";
import { isEqual } from "lodash";
import * as Catalog from "../catalog";
import * as Container from "../container";
import { Derived, Signal, Signals, signal } from "../signals";

export type VideoProps = {
	canvas?: HTMLCanvasElement;
	paused?: boolean;
	latency?: number;
};

// An component to render a video to a canvas.
export class Video {
	// The source of video frames, also responsible for switching between video tracks.
	source: VideoSource;

	// The canvas to render the video to.
	canvas: Signal<HTMLCanvasElement | undefined>;

	// Whether the video is paused.
	paused: Signal<boolean>;

	// The latency in milliseconds.
	latency: Signal<number>;

	#animate?: number;

	#ctx!: Derived<CanvasRenderingContext2D | undefined>;
	#signals = new Signals();

	constructor(source: VideoSource, props?: VideoProps) {
		this.source = source;
		this.canvas = signal(props?.canvas);
		this.paused = signal(props?.paused ?? false);
		this.latency = signal(props?.latency ?? 50);

		this.#ctx = this.#signals.derived(() => this.canvas.get()?.getContext("2d") ?? undefined);
		this.#signals.effect(() => this.#schedule());
		this.#signals.effect(() => this.#runEnabled());

		// Proxy the latency signal.
		this.#signals.effect(() => {
			this.source.latency.set(this.latency.get());
		});
	}

	// Detect when video should be downloaded.
	#runEnabled() {
		const canvas = this.canvas.get();
		if (!canvas) return;

		const paused = this.paused.get();
		if (paused) return;

		// Detect when the canvas is not visible.
		const observer = new IntersectionObserver(
			(entries) => {
				for (const entry of entries) {
					this.source.enabled.set(entry.isIntersecting);
				}
			},
			{
				// fire when even a small part is visible
				threshold: 0.01,
			},
		);

		observer.observe(canvas);

		return () => {
			observer.disconnect();
			this.source.enabled.set(false);
		};
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

export type VideoSourceProps = {
	broadcast?: Moq.BroadcastConsumer;
	available?: Catalog.Video[];
	enabled?: boolean;
	latency?: number;
};

// Responsible for switching between video tracks and buffering frames.
export class VideoSource {
	broadcast: Signal<Moq.BroadcastConsumer | undefined>;
	enabled: Signal<boolean>; // Don't download any longer

	// The maximum latency in milliseconds.
	// The larger the value, the more tolerance we have for network jitter.
	// This should be at least 1 frame for smoother playback.
	latency: Signal<number>;
	tracks: Signal<Catalog.Video[]>;

	selected: Derived<Catalog.Video | undefined>;

	#frames: VideoFrame[] = [];

	// The difference between the wall clock units and the timestamp units, in seconds.
	#ref?: number;

	#signals = new Signals();

	constructor(props?: VideoSourceProps) {
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
		const media = new Container.Decoder(sub);

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
			...selected,
			codedHeight: selected.dimensions?.height,
			codedWidth: selected.dimensions?.width,
			displayAspectHeight: selected.displayRatio?.height,
			displayAspectWidth: selected.displayRatio?.width,
			description: selected.description ? Buffer.from(selected.description, "hex") : undefined,
			optimizeForLatency: selected.optimizeForLatency ?? true,
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
		const maxLatency = this.#maxLatencyUs();

		// Advance the reference timestamp if we need to.
		// We want the newest frame (last) to be be scheduled at most maxLatency in the future.
		// NOTE: This never goes backwards, so clock drift will cause issues.
		const last = this.#frames.at(-1);
		if (!last) {
			return;
		}

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

	#maxLatencyUs() {
		let maxLatency = this.latency.peek() * 1000;
		const first = this.#frames.at(0);
		const last = this.#frames.at(-1);

		if (first && last) {
			// Compute the average duration of a frame
			const duration = (last.timestamp - first.timestamp) / this.#frames.length;
			maxLatency += duration;
		}

		return maxLatency;
	}

	#prune() {
		const maxLatency = this.#maxLatencyUs();

		let first = this.#frames.at(0);
		const last = this.#frames.at(-1);

		while (first && last) {
			if (last.timestamp - first.timestamp <= maxLatency) {
				break;
			}

			const frame = this.#frames.shift();
			frame?.close();

			first = this.#frames.at(0);
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
