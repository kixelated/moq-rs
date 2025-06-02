import { Buffer } from "buffer";
import * as Moq from "@kixelated/moq";
import { Memo, Signal, Signals, signal } from "@kixelated/signals";
import { isEqual } from "lodash";
import * as Catalog from "../catalog";
import * as Container from "../container";

export type VideoRendererProps = {
	canvas?: HTMLCanvasElement;
	paused?: boolean;
};

// An component to render a video to a canvas.
export class VideoRenderer {
	// The source of video frames, also responsible for switching between video tracks.
	source: Video;

	// The canvas to render the video to.
	canvas: Signal<HTMLCanvasElement | undefined>;

	// Whether the video is paused.
	paused: Signal<boolean>;

	#animate?: number;

	#ctx!: Memo<CanvasRenderingContext2D | undefined>;
	#signals = new Signals();

	constructor(source: Video, props?: VideoRendererProps) {
		this.source = source;
		this.canvas = signal(props?.canvas);
		this.paused = signal(props?.paused ?? false);

		this.#ctx = this.#signals.memo(
			() => this.canvas.get()?.getContext("2d", { desynchronized: true }) ?? undefined,
		);
		this.#signals.effect(() => this.#schedule());
		this.#signals.effect(() => this.#runEnabled());
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

		const { frame, lag } = this.source.frame(now) ?? {};
		if (frame) {
			ctx.canvas.width = frame.displayWidth;
			ctx.canvas.height = frame.displayHeight;
			ctx.drawImage(frame, 0, 0, ctx.canvas.width, ctx.canvas.height);
		}

		// Draw a loading icon when the lag 2+ seconds
		// TODO expose this as a signal
		const spinner = Math.min(Math.max(((lag ?? 0) - 2000) / (4000 - 2000), 0), 1);
		if (spinner > 0) {
			const spinnerSize = 64;
			const spinnerX = ctx.canvas.width / 2 - spinnerSize / 2;
			const spinnerY = ctx.canvas.height / 2 - spinnerSize / 2;
			const angle = ((now % 1000) / 1000) * 2 * Math.PI;

			ctx.save();
			ctx.translate(spinnerX + spinnerSize / 2, spinnerY + spinnerSize / 2);
			ctx.rotate(angle);

			ctx.beginPath();
			ctx.arc(0, 0, spinnerSize / 2 - 2, 0, Math.PI * 1.5); // crude 3/4 arc
			ctx.lineWidth = 8;
			ctx.strokeStyle = `rgba(255, 255, 255, ${spinner})`;
			ctx.stroke();

			ctx.restore();
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

export type VideoProps = {
	enabled?: boolean;
};

// Responsible for switching between video tracks and buffering frames.
export class Video {
	broadcast: Signal<Moq.BroadcastConsumer | undefined>;
	enabled: Signal<boolean>; // Don't download any longer
	catalog: Signal<Catalog.Root | undefined>;
	selected: Memo<Catalog.Video | undefined>;
	active: Memo<boolean>;

	// Unfortunately, browsers don't let us hold on to multiple VideoFrames.
	// TODO To support higher latencies, keep around the encoded data and decode on demand.
	// ex. Firefox only allows 2 outstanding VideoFrames at a time.
	// We hold a second frame buffered as a crude way to introduce latency to sync with audio.
	#current?: VideoFrame;
	#next?: VideoFrame;

	// The largest timestamp - now that we've ever seen.
	// This is used to calculate the jitter/lag.
	#ref?: DOMHighResTimeStamp;

	#signals = new Signals();

	constructor(
		broadcast: Signal<Moq.BroadcastConsumer | undefined>,
		catalog: Signal<Catalog.Root | undefined>,
		props?: VideoProps,
	) {
		this.broadcast = broadcast;
		this.catalog = catalog;
		this.enabled = signal(props?.enabled ?? false);

		// TODO use isConfigSupported
		this.selected = this.#signals.memo(() => this.catalog.get()?.video?.[0], {
			equals: (a, b) => isEqual(a, b),
		});
		this.active = this.#signals.memo(() => this.selected.get() !== undefined);

		this.#signals.effect(() => this.#init());
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
				if (!this.#current) {
					this.#current = frame;
					return;
				}

				if (!this.#next) {
					this.#next = frame;
					return;
				}

				this.#current?.close();
				this.#current = this.#next;
				this.#next = frame;
			},
			// TODO bubble up error
			error: (error) => {
				console.error(error);
				this.close();
			},
		});

		const config = selected.config;

		decoder.configure({
			...config,
			description: config.description ? Buffer.from(config.description, "hex") : undefined,
			optimizeForLatency: config.optimizeForLatency ?? true,
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

	// Returns the closest frame to the given timestamp and the lag.
	frame(now: DOMHighResTimeStamp): { frame: VideoFrame; lag: DOMHighResTimeStamp } | undefined {
		for (;;) {
			if (!this.#current) return;

			const ref = this.#current.timestamp / 1000 - now;
			if (!this.#ref || ref > this.#ref) {
				this.#ref = ref;
			}

			const lag = this.#ref - ref;

			// Determine if we should skip to the next frame or not.
			// If the lag is greater than the frame duration, then sure let's do it.
			// This should result in marginally smoother playback, especially if there's a B-frame.
			if (!this.#next || lag <= (this.#next.timestamp - this.#current.timestamp) / 1000) {
				return {
					frame: this.#current,
					lag,
				};
			}

			this.#current?.close();
			this.#current = this.#next;
			this.#next = undefined;
		}
	}

	close() {
		this.#current?.close();
		this.#current = undefined;
		this.#next?.close();
		this.#next = undefined;
		this.#signals.close();
	}
}
