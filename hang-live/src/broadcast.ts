import * as Watch from "@kixelated/hang/watch";
import * as Moq from "@kixelated/moq";

import { Bounds } from "./bounds";
import { Vector } from "./vector";

export class Broadcast {
	watch: Watch.Broadcast;
	room: string;

	audio: Watch.AudioEmitter;
	audioPanner?: StereoPannerNode;
	audioLeft?: AnalyserNode;
	audioRight?: AnalyserNode;

	video: Watch.VideoRenderer;

	bounds: Bounds;
	scale = 1.0; // 1 is 100%
	velocity = Vector.create(0, 0); // in pixels per ?

	targetPosition = Vector.create(0.5, 0.5); // in 0-1
	targetScale = 1.0; // 1 is 100%
	targetSize: Vector; // in pixels

	constructor(broadcast: Moq.BroadcastReader, room: string) {
		this.watch = new Watch.Broadcast(broadcast);
		this.room = room;

		this.video = new Watch.VideoRenderer();
		this.video.broadcast = this.watch.video;

		this.audio = new Watch.AudioEmitter();
		this.audio.onInit = (ctx: AudioContext, node: AudioNode) => {
			if (node.channelCount >= 2) {
				this.audioPanner = new StereoPannerNode(ctx, {
					channelCount: node.channelCount,
				});
				const splitter = new ChannelSplitterNode(ctx, {
					channelCount: node.channelCount,
					numberOfOutputs: 2,
				});

				this.audioLeft = new AnalyserNode(ctx, { fftSize: 256 });
				this.audioRight = new AnalyserNode(ctx, { fftSize: 256 });

				splitter.connect(this.audioLeft, 0);
				splitter.connect(this.audioRight, 1);

				node.connect(this.audioPanner);
				this.audioPanner.connect(splitter);
				this.audioPanner.connect(ctx.destination);
			} else {
				this.audioPanner = undefined;
				this.audioLeft = new AnalyserNode(ctx, { fftSize: 256 });
				this.audioRight = this.audioLeft;

				node.connect(this.audioLeft);
				node.connect(ctx.destination); // output to the speakers
			}
		};

		this.audio.broadcast = this.watch.audio;

		this.targetSize = Vector.create(128, 128);
		this.bounds = new Bounds(Vector.create(0, 0), this.targetSize);
	}

	renderAudio(ctx: CanvasRenderingContext2D, _now: DOMHighResTimeStamp) {
		if (!this.audioLeft || !this.audioRight) {
			return;
		}

		const bounds = this.bounds;

		ctx.translate(bounds.position.x, bounds.position.y);

		// Round down the height to the nearest power of 2.
		const bars = Math.max(2 ** Math.floor(Math.log2(bounds.size.y / 4)), 32);
		const barHeight = bounds.size.y / bars;
		const barData = new Uint8Array(bars); // TODO reuse a buffer.
		const barScale = 4 * this.scale;

		this.audioLeft.fftSize = bars;
		this.audioLeft.getByteFrequencyData(barData);

		for (let i = 0; i < bars / 2; i++) {
			const power = barData[i] / 255;
			const hue = 2 ** power * 100 + 135;
			const barWidth = 4 ** power * barScale;

			ctx.fillStyle = `hsla(${hue}, 80%, 40%, ${power})`;
			ctx.fillRect(-barWidth, bounds.size.y / 2 - (i + 1) * barHeight, barWidth, barHeight + 0.1);
			ctx.fillRect(-barWidth, bounds.size.y / 2 + i * barHeight, barWidth, barHeight + 0.1);
		}

		this.audioRight.fftSize = bars;
		this.audioRight.getByteFrequencyData(barData);

		for (let i = 0; i < bars / 2; i++) {
			const power = barData[i] / 255;
			const hue = 2 ** power * 100 + 135;
			const barWidth = 4 ** power * barScale;

			ctx.fillStyle = `hsla(${hue}, 80%, 40%, ${power})`;
			ctx.fillRect(bounds.size.x, bounds.size.y / 2 - (i + 1) * barHeight, barWidth, barHeight + 0.1);
			ctx.fillRect(bounds.size.x, bounds.size.y / 2 + i * barHeight, barWidth, barHeight + 0.1);
		}
	}

	renderVideo(
		ctx: CanvasRenderingContext2D,
		now: DOMHighResTimeStamp,
		modifiers: { dragging?: boolean; hovering?: boolean },
	) {
		const bounds = this.bounds;

		ctx.translate(bounds.position.x, bounds.position.y);
		ctx.fillStyle = "#000";

		const frame = this.video.frame(now);
		if (frame) {
			// Check if the frame size has changed.
			this.targetSize = Vector.create(frame.displayWidth, frame.displayHeight);
			ctx.save();

			if (modifiers.dragging) {
				// Apply an opacity to the image.
				ctx.globalAlpha = 0.7;
			}

			// Create a rounded rectangle path
			const radius = 8;
			const w = bounds.size.x;
			const h = bounds.size.y;

			ctx.beginPath();
			ctx.moveTo(radius, 0);
			ctx.lineTo(w - radius, 0);
			ctx.quadraticCurveTo(w, 0, w, radius);
			ctx.lineTo(w, h - radius);
			ctx.quadraticCurveTo(w, h, w - radius, h);
			ctx.lineTo(radius, h);
			ctx.quadraticCurveTo(0, h, 0, h - radius);
			ctx.lineTo(0, radius);
			ctx.quadraticCurveTo(0, 0, radius, 0);
			ctx.closePath();

			// Clip and draw the image
			ctx.clip();

			ctx.drawImage(frame, 0, 0, bounds.size.x, bounds.size.y);
			ctx.restore();
		} else {
			ctx.fillRect(0, 0, bounds.size.x, bounds.size.y);
		}

		if (modifiers.hovering) {
			/*
			this.#ctx.lineWidth = 2;
			this.#ctx.strokeStyle = "white";
			this.#ctx.strokeRect(0, 0, bounds.size.x, bounds.size.y);
			*/
		}

		const name = this.watch.broadcast.path.slice(this.room.length + 1);

		ctx.font = "12px sans-serif";
		ctx.lineWidth = 3;
		ctx.strokeStyle = "black";
		ctx.strokeText(name, 6, 16);

		ctx.fillStyle = "white";
		ctx.fillText(name, 6, 16);
		ctx.restore();

		// Draw target for debugging
		/*
		this.#ctx.beginPath();
		this.#ctx.arc(
			broadcast.targetPosition.x * this.#ctx.canvas.width,
			broadcast.targetPosition.y * this.#ctx.canvas.height,
			4,
			0,
			2 * Math.PI,
		);
		this.#ctx.fillStyle = "rgba(255, 0, 0, 0.5)";
		this.#ctx.fill();
		*/
	}

	close() {
		this.watch.close();
		this.video.close();
		this.audio.close();
	}
}
