import { Signals } from "@kixelated/hang/signals";
import * as Watch from "@kixelated/hang/watch";

import { Bounds } from "./bounds";
import { Vector } from "./vector";

export class Broadcast {
	watch: Watch.Broadcast;
	name: string;

	audio: Watch.Audio;
	audioEmitter: Watch.AudioEmitter;
	audioPanner?: StereoPannerNode;
	audioLeft?: AnalyserNode;
	audioRight?: AnalyserNode;

	video: Watch.Video;

	bounds: Bounds;
	scale = 1.0; // 1 is 100%
	velocity = Vector.create(0, 0); // in pixels per ?

	targetPosition = Vector.create(0.5, 0.5); // in 0-1
	targetScale = 1.0; // 1 is 100%
	targetSize: Vector; // in pixels

	// 1 when a video frame is fully rendered, 0 when it's not.
	fade = 0;

	#signals = new Signals();

	constructor(watch: Watch.Broadcast, name: string) {
		this.watch = watch;
		this.name = name;

		this.video = this.watch.video;
		this.video.enabled.set(true);

		this.audio = this.watch.audio;
		this.audioEmitter = new Watch.AudioEmitter({ source: this.audio });

		this.targetSize = Vector.create(128, 128);
		this.bounds = new Bounds(Vector.create(0, 0), this.targetSize);

		this.#signals.effect(() => this.#setupAudio());
	}

	#setupAudio() {
		const audio = this.audioEmitter.context.get();
		if (!audio) return;

		const { root: context, gain } = audio;

		if (gain.channelCount >= 2) {
			this.audioPanner = new StereoPannerNode(context, {
				channelCount: gain.channelCount,
			});
			const splitter = new ChannelSplitterNode(context, {
				channelCount: gain.channelCount,
				numberOfOutputs: 2,
			});

			this.audioLeft = new AnalyserNode(context, { fftSize: 256 });
			this.audioRight = new AnalyserNode(context, { fftSize: 256 });

			splitter.connect(this.audioLeft, 0);
			splitter.connect(this.audioRight, 1);

			gain.connect(this.audioPanner);
			this.audioPanner.connect(splitter);
			this.audioPanner.connect(context.destination);
		} else {
			this.audioPanner = undefined;
			this.audioLeft = new AnalyserNode(context, { fftSize: 256 });
			this.audioRight = this.audioLeft;

			gain.connect(this.audioLeft);
			gain.connect(context.destination); // output to the speakers
		}

		return () => {
			this.audioLeft?.disconnect();
			this.audioRight?.disconnect();
			this.audioPanner?.disconnect();
		};
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
		const barScale = 8 * this.scale;

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
		modifiers?: { dragging?: boolean; hovering?: boolean },
	) {
		const bounds = this.bounds;
		ctx.save();

		ctx.translate(bounds.position.x, bounds.position.y);
		ctx.fillStyle = "#000";

		// Create a rounded rectangle path
		const radius = 8 * this.scale;
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

		// Apply an opacity to the image.
		if (modifiers?.dragging) {
			ctx.globalAlpha = 0.7;
		}

		const closest = this.video.frame(now);

		// Check if the frame size has changed.
		if (closest && this.video.selected.peek()) {
			this.targetSize = Vector.create(closest.frame.displayWidth, closest.frame.displayHeight);
			this.fade = Math.min(this.fade + 0.05, 1);
		} else {
			this.targetSize = Vector.create(128, 128);
			this.fade = Math.max(this.fade - 0.01, 0);
		}

		if (closest) {
			ctx.save();
			ctx.globalAlpha *= this.fade;

			// Compute grayscale level based on how late the frame is.
			const lag = Math.min(Math.max((closest.lag - 2000) / (5000 - 2000), 0), 1);
			if (lag > 0) {
				ctx.filter = `grayscale(${lag})`;
			}

			ctx.imageSmoothingEnabled = true;
			ctx.drawImage(closest.frame, 0, 0, bounds.size.x, bounds.size.y);
			ctx.restore();

			if (lag > 0) {
				const spinnerSize = 32 * this.scale;
				const spinnerX = bounds.size.x / 2 - spinnerSize / 2;
				const spinnerY = bounds.size.y / 2 - spinnerSize / 2;
				const angle = ((now % 1000) / 1000) * 2 * Math.PI;

				ctx.save();
				ctx.translate(spinnerX + spinnerSize / 2, spinnerY + spinnerSize / 2);
				ctx.rotate(angle);

				ctx.beginPath();
				ctx.arc(0, 0, spinnerSize / 2 - 2, 0, Math.PI * 1.5); // crude 3/4 arc
				ctx.lineWidth = 4 * this.scale;
				ctx.strokeStyle = `hsla(290, 80%, 40%, ${lag})`;
				ctx.stroke();

				ctx.restore();
			}
		}

		if (this.fade < 1) {
			ctx.save();
			ctx.globalAlpha *= 1 - this.fade;
			ctx.fillRect(0, 0, bounds.size.x, bounds.size.y);
			ctx.restore();
		}

		//if (modifiers.hovering) {
		//ctx.lineWidth = 2 * this.scale;
		//ctx.strokeStyle = "white";
		//ctx.strokeRect(0, 0, bounds.size.x, bounds.size.y);
		//}

		ctx.font = `${24 * this.scale}px sans-serif`;
		ctx.lineWidth = 3 * this.scale;
		ctx.strokeStyle = "black";
		ctx.strokeText(this.name, 12 * this.scale, 32 * this.scale);

		ctx.fillStyle = "white";
		ctx.fillText(this.name, 12 * this.scale, 32 * this.scale);

		ctx.restore();

		// Draw target for debugging
		/*
		ctx.beginPath();
		ctx.arc(
			this.targetPosition.x * ctx.canvas.width,
			this.targetPosition.y * ctx.canvas.height,
			4 * this.scale,
			0,
			2 * Math.PI,
		);
		ctx.fillStyle = "rgba(255, 0, 0, 0.5)";
		ctx.fill();
		*/
	}

	rip() {
		this.targetPosition = Vector.create(Math.random(), Math.random());
	}

	close() {
		this.#signals.close();
		this.watch.close();
		this.video.close();
		this.audio.close();
		this.audioEmitter.close();
	}
}
