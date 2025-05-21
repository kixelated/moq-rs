import { Buffer } from "buffer";
import * as Moq from "@kixelated/moq";
import { Derived, Signal, Signals, signal } from "@kixelated/signals";
import { isEqual } from "lodash";
import * as Catalog from "../catalog";
import * as Container from "../container";

export type WatchVideoProps = {
	canvas?: HTMLCanvasElement;
	paused?: boolean;
};

// An component to render a video to a canvas.
export class WatchVideo {
	// The source of video frames, also responsible for switching between video tracks.
	source: WatchVideoSource;

	// The canvas to render the video to.
	canvas: Signal<HTMLCanvasElement | undefined>;

	// Whether the video is paused.
	paused: Signal<boolean>;

	#animate?: number;

	#ctx!: Derived<CanvasRenderingContext2D | undefined>;
	#signals = new Signals();

	constructor(source: WatchVideoSource, props?: WatchVideoProps) {
		this.source = source;
		this.canvas = signal(props?.canvas);
		this.paused = signal(props?.paused ?? false);

		this.#ctx = this.#signals.derived(
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

	#render() {
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

		const frame = this.source.frame;
		if (frame) {
			ctx.canvas.width = frame.displayWidth;
			ctx.canvas.height = frame.displayHeight;
			ctx.drawImage(frame, 0, 0, ctx.canvas.width, ctx.canvas.height);
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

export type WatchVideoSourceProps = {
	broadcast?: Moq.BroadcastConsumer;
	available?: Catalog.Video[];
	enabled?: boolean;
};

// Responsible for switching between video tracks and buffering frames.
export class WatchVideoSource {
	broadcast: Signal<Moq.BroadcastConsumer | undefined>;
	enabled: Signal<boolean>; // Don't download any longer
	tracks: Signal<Catalog.Video[]>;
	selected: Derived<Catalog.Video | undefined>;

	// Unfortunately, browsers don't let us hold on to multiple VideoFrames.
	// TODO To support higher latencies, keep around the encoded data and decode on demand.
	// ex. Firefox only allows 2 outstanding VideoFrames at a time.
	frame?: VideoFrame;

	// We hold a second frame buffered as a crude way to introduce latency to sync with audio.
	#next?: VideoFrame;

	#signals = new Signals();

	constructor(props?: WatchVideoSourceProps) {
		this.broadcast = signal(props?.broadcast);
		this.tracks = signal(props?.available ?? []);
		this.enabled = signal(props?.enabled ?? false);

		this.selected = this.#signals.derived(() => this.tracks.get().at(0), {
			equals: (a, b) => isEqual(a, b),
		});
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
				if (!this.frame) {
					this.frame = frame;
				} else if (!this.#next) {
					this.#next = frame;
				} else {
					this.frame?.close();
					this.frame = this.#next;
					this.#next = frame;
				}
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

	close() {
		this.frame?.close();
		this.frame = undefined;
		this.#next?.close();
		this.#next = undefined;
		this.#signals.close();
	}
}
