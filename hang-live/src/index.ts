import * as Moq from "@kixelated/moq";
import * as Watch from "@kixelated/hang/watch";

import { Vector } from "./util/vector";
import { Bounds } from "./util/bounds";

export class Room {
	// The connection to the server.
	connection: Moq.Connection;
	#announced: Moq.AnnouncedReader;
	#broadcasts = new Map<string, Broadcast>();

	canvas: HTMLCanvasElement;
	#ctx: CanvasRenderingContext2D;

	#hovering?: Broadcast;
	#dragging?: Broadcast;
	#scale = 1.0;

	#muted = true;

	constructor(connection: Moq.Connection, room: string, canvas: HTMLCanvasElement) {
		this.connection = connection;
		this.canvas = canvas;

		this.#announced = connection.announced(room);

		const ctx = canvas.getContext("2d");
		if (!ctx) {
			throw new Error("Failed to get canvas context");
		}

		this.#ctx = ctx;

		canvas.addEventListener("resize", () => {
			this.#updateScale();
		});

		canvas.addEventListener("mousedown", (e) => {
			const rect = canvas.getBoundingClientRect();
			const mouse = Vector.create(e.clientX - rect.left, e.clientY - rect.top);

			this.#dragging = this.#broadcastAt(mouse);
			if (this.#dragging) {
				canvas.style.cursor = "grabbing";
			}
		});

		canvas.addEventListener("mousemove", (e) => {
			const rect = canvas.getBoundingClientRect();
			const mouse = Vector.create(e.clientX - rect.left, e.clientY - rect.top);

			if (this.#dragging) {
				this.#dragging.targetPosition = Vector.create(mouse.x / this.canvas.width, mouse.y / this.canvas.height);
			} else {
				this.#hovering = this.#broadcastAt(mouse);
				if (this.#hovering) {
					canvas.style.cursor = "grab";
				} else {
					canvas.style.cursor = "default";
				}
			}
		});

		canvas.addEventListener("mouseup", () => {
			if (this.#dragging) {
				this.#dragging = undefined;
				this.#hovering = undefined;
				canvas.style.cursor = "default";
			}
		});

		canvas.addEventListener("mouseleave", () => {
			if (this.#dragging) {
				this.#dragging = undefined;
				this.#hovering = undefined;
				canvas.style.cursor = "default";
			}
		});

		// We require user interaction to unmute the audio.
		canvas.addEventListener(
			"click",
			() => {
				this.#muted = false;
				for (const broadcast of this.#broadcasts.values()) {
					broadcast.audio.muted = this.#muted;
				}
			},
			{ once: true },
		);

		canvas.addEventListener(
			"wheel",
			(e) => {
				e.preventDefault(); // Prevent scroll

				let broadcast = this.#dragging;
				if (!broadcast) {
					const rect = canvas.getBoundingClientRect();
					const mouse = Vector.create(e.clientX - rect.left, e.clientY - rect.top);

					broadcast = this.#broadcastAt(mouse);
					if (!broadcast) return;

					this.#hovering = broadcast;
				}

				const scale = e.deltaY * 0.001;
				if (scale < 0) {
					canvas.style.cursor = "zoom-out";
				} else if (scale > 0) {
					canvas.style.cursor = "zoom-in";
				}

				broadcast.targetScale = Math.max(Math.min(broadcast.targetScale + scale, 4), 0.25);
			},
			{ passive: false },
		);

		this.#run().finally(() => this.close());
		requestAnimationFrame(this.#tick.bind(this));
	}

	#broadcastAt(point: Vector) {
		// We need to iterate in reverse order to respect the z-index.
		// TODO: Short-circuit on the first result, but that requires a reverse iterator.
		let result: Broadcast | undefined;

		for (const broadcast of this.#broadcasts.values()) {
			if (broadcast.bounds.contains(point)) {
				result = broadcast;
			}
		}

		return result;
	}

	async #run() {
		for (;;) {
			const update = await this.#announced.next();

			// We're donezo.
			if (!update) break;

			if (update.active) {
				this.#startBroadcast(update.broadcast);
			} else {
				this.#stopBroadcast(update.broadcast);
			}

			this.#updateScale();
		}
	}

	#startBroadcast(path: string) {
		const targetPosition = Vector.create(Math.random(), Math.random());

		const offset = Vector.create(targetPosition.x - 0.5, targetPosition.y - 0.5)
			.normalize()
			.mult(Math.sqrt(this.canvas.width ** 2 + this.canvas.height ** 2));

		// Follow the unit vector of the target position and go outside the screen.
		const startPosition = Vector.create(
			targetPosition.x * this.canvas.width,
			targetPosition.y * this.canvas.height,
		).add(offset);

		const broadcast = new Broadcast(this.connection.consume(path));
		broadcast.targetPosition = targetPosition;
		broadcast.bounds.position = startPosition;
		broadcast.audio.muted = this.#muted;

		this.#broadcasts.set(path, broadcast);
	}

	#stopBroadcast(path: string) {
		const broadcast = this.#broadcasts.get(path);
		if (!broadcast) return;

		broadcast.close();
		this.#broadcasts.delete(path);
	}

	#tick(now: DOMHighResTimeStamp) {
		this.#updateScale();

		const broadcasts = Array.from(this.#broadcasts.values());

		for (const broadcast of broadcasts) {
			broadcast.scale += (broadcast.targetScale - broadcast.scale) * 0.1;
			broadcast.bounds.size = broadcast.targetSize.mult(broadcast.scale * this.#scale);

			// Slowly slow down the velocity.
			broadcast.velocity = broadcast.velocity.mult(0.5);

			// Guide the body towards the target position with a bit of force.
			const target = Vector.create(
				broadcast.targetPosition.x * this.canvas.width,
				broadcast.targetPosition.y * this.canvas.height,
			);

			const middle = broadcast.bounds.middle();

			// Make sure the target wouldn't cause us to be outside the canvas.
			const width = broadcast.bounds.size.x;
			const height = broadcast.bounds.size.y;

			target.x = Math.max(width / 2, Math.min(target.x, this.canvas.width - width / 2));
			target.y = Math.max(height / 2, Math.min(target.y, this.canvas.height - height / 2));

			const force = target.sub(middle);
			broadcast.velocity = broadcast.velocity.add(force);

			const left = broadcast.bounds.position.x;
			const right = broadcast.bounds.position.x + broadcast.bounds.size.x;
			const top = broadcast.bounds.position.y;
			const bottom = broadcast.bounds.position.y + broadcast.bounds.size.y;

			if (left < 0) {
				broadcast.velocity.x += -left;
			} else if (right > this.canvas.width) {
				broadcast.velocity.x += this.canvas.width - right;
			}

			if (top < 0) {
				broadcast.velocity.y += -top;
			} else if (bottom > this.canvas.height) {
				broadcast.velocity.y += this.canvas.height - bottom;
			}
		}

		// Loop over again, this time checking for collisions.
		for (let i = 0; i < broadcasts.length; i++) {
			for (let j = i + 1; j < broadcasts.length; j++) {
				const a = broadcasts[i];
				const b = broadcasts[j];

				// Compute the intersection rectangle.
				const intersection = a.bounds.intersects(b.bounds);
				if (!intersection) {
					continue;
				}

				// Repel each other based on the size of the intersection.
				const strength = intersection.area() / a.bounds.area(); // TODO what about b.area()?
				let force = a.bounds.middle().sub(b.bounds.middle()).mult(strength);

				if (this.#dragging !== a && this.#dragging !== b) {
					force = force.mult(10);
				}

				a.velocity = a.velocity.add(force);
				b.velocity = b.velocity.sub(force);
			}
		}

		// Finally apply the velocity to the position.
		for (let i = 0; i < broadcasts.length; i++) {
			const broadcast = broadcasts[i];
			broadcast.bounds = broadcast.bounds.add(broadcast.velocity.div(50));

			// Update the audio panner with the new position.
			if (broadcast.audioPanner) {
				broadcast.audioPanner.pan.value = Math.min(
					Math.max((2 * broadcast.bounds.middle().x) / this.canvas.width - 1, -1),
					1,
				);
			}
		}

		this.#render(now);

		requestAnimationFrame(this.#tick.bind(this));
	}

	#render(now: DOMHighResTimeStamp) {
		this.#ctx.clearRect(0, 0, this.#ctx.canvas.width, this.#ctx.canvas.height);

		for (const broadcast of this.#broadcasts.values()) {
			this.#renderAudio(now, broadcast);
		}

		for (const broadcast of this.#broadcasts.values()) {
			if (this.#dragging !== broadcast) {
				this.#renderBroadcast(now, broadcast);
			}
		}

		// Render the dragging broadcast last so it's on top.
		if (this.#dragging) {
			this.#ctx.save();
			this.#ctx.fillStyle = "rgba(0, 0, 0, 0.5)";
			this.#renderBroadcast(now, this.#dragging);
			this.#ctx.restore();
		}
	}

	#renderAudio(_now: DOMHighResTimeStamp, broadcast: Broadcast) {
		if (!broadcast.audioLeft || !broadcast.audioRight) {
			return;
		}

		const bounds = broadcast.bounds;

		this.#ctx.save();
		this.#ctx.translate(bounds.position.x, bounds.position.y);

		// Round down the height to the nearest power of 2.
		const bars = Math.max(2 ** Math.floor(Math.log2(bounds.size.y / 4)), 32);
		const barHeight = bounds.size.y / bars;
		const barData = new Uint8Array(bars); // TODO reuse a buffer.
		const barScale = 4 * broadcast.scale;

		if (broadcast.audioLeft) {
			broadcast.audioLeft.fftSize = bars;
			broadcast.audioLeft.getByteFrequencyData(barData);
		}

		for (let i = 0; i < bars / 2; i++) {
			const power = barData[i] / 255;
			const hue = 2 ** power * 100 + 135;
			const barWidth = 3 ** power * barScale;

			this.#ctx.fillStyle = `hsla(${hue}, 80%, 40%, ${power})`;
			this.#ctx.fillRect(-barWidth, bounds.size.y / 2 - (i + 1) * barHeight, barWidth, barHeight + 0.1);
			this.#ctx.fillRect(-barWidth, bounds.size.y / 2 + i * barHeight, barWidth, barHeight + 0.1);
		}

		if (broadcast.audioRight) {
			broadcast.audioRight.fftSize = bars;
			broadcast.audioRight.getByteFrequencyData(barData);
		}

		for (let i = 0; i < bars / 2; i++) {
			const power = barData[i] / 255;
			const hue = 2 ** power * 100 + 135;
			const barWidth = 3 ** power * barScale;

			this.#ctx.fillStyle = `hsla(${hue}, 80%, 40%, ${power})`;
			this.#ctx.fillRect(bounds.size.x, bounds.size.y / 2 - (i + 1) * barHeight, barWidth, barHeight + 0.1);
			this.#ctx.fillRect(bounds.size.x, bounds.size.y / 2 + i * barHeight, barWidth, barHeight + 0.1);
		}

		this.#ctx.restore();
	}

	#renderBroadcast(now: DOMHighResTimeStamp, broadcast: Broadcast) {
		const bounds = broadcast.bounds;

		this.#ctx.save();
		this.#ctx.font = "12px sans-serif";
		this.#ctx.translate(bounds.position.x, bounds.position.y);
		this.#ctx.fillStyle = "#000";

		const frame = broadcast.video.frame(now);
		if (frame) {
			// Check if the frame size has changed and recompute the scale.
			if (broadcast.targetSize.x !== frame.displayWidth || broadcast.targetSize.y !== frame.displayHeight) {
				broadcast.targetSize = Vector.create(frame.displayWidth, frame.displayHeight);
				this.#updateScale();
			}

			if (this.#dragging === broadcast) {
				// Apply an opacity to the image.
				this.#ctx.globalAlpha = 0.7;
			}

			this.#ctx.drawImage(frame, 0, 0, bounds.size.x, bounds.size.y);
			this.#ctx.globalAlpha = 1.0;
		} else {
			this.#ctx.fillRect(0, 0, bounds.size.x, bounds.size.y);
		}

		// Round down the height to the nearest power of 2.
		const bars = Math.max(2 ** Math.floor(Math.log2(bounds.size.y / 4)), 32);
		const barHeight = bounds.size.y / bars;
		const barData = new Uint8Array(bars); // TODO reuse a buffer.
		const barScale = 2 * broadcast.scale;

		if (broadcast.audioLeft) {
			broadcast.audioLeft.fftSize = bars;
			broadcast.audioLeft.getByteFrequencyData(barData);
		}

		for (let i = 0; i < bars / 2; i++) {
			const power = barData[i] / 255;
			const hue = 2 ** power * 100 + 135;
			const barWidth = 3 ** power * barScale;

			this.#ctx.fillStyle = `hsla(${hue}, 80%, 40%, ${power})`;
			this.#ctx.fillRect(-barWidth, bounds.size.y / 2 - (i + 1) * barHeight, barWidth, barHeight + 0.1);
			this.#ctx.fillRect(-barWidth, bounds.size.y / 2 + i * barHeight, barWidth, barHeight + 0.1);
		}

		if (broadcast.audioRight) {
			broadcast.audioRight.fftSize = bars;
			broadcast.audioRight.getByteFrequencyData(barData);
		}

		for (let i = 0; i < bars / 2; i++) {
			const power = barData[i] / 255;
			const hue = 2 ** power * 100 + 135;
			const barWidth = 3 ** power * barScale;

			this.#ctx.fillStyle = `hsla(${hue}, 80%, 40%, ${power})`;
			this.#ctx.fillRect(bounds.size.x, bounds.size.y / 2 - (i + 1) * barHeight, barWidth, barHeight + 0.1);
			this.#ctx.fillRect(bounds.size.x, bounds.size.y / 2 + i * barHeight, barWidth, barHeight + 0.1);
		}

		if (this.#hovering === broadcast) {
			//this.#ctx.lineWidth = 1;
			//this.#ctx.strokeStyle = "white";
			//this.#ctx.strokeRect(0, 0, bounds.size.x, bounds.size.y);
		}

		this.#ctx.lineWidth = 3;
		this.#ctx.strokeStyle = "black";
		this.#ctx.strokeText(broadcast.watch.broadcast.path, 4, 14);

		this.#ctx.fillStyle = "white";
		this.#ctx.fillText(broadcast.watch.broadcast.path, 4, 14);
		this.#ctx.restore();

		// Draw target for debugging
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
	}

	#updateScale() {
		const canvasArea = this.canvas.width * this.canvas.height;
		let broadcastArea = 0;

		for (const broadcast of this.#broadcasts.values()) {
			broadcastArea += broadcast.targetSize.x * broadcast.targetSize.y;
		}

		const fillRatio = broadcastArea / canvasArea;
		const targetFill = 0.5;

		this.#scale = Math.sqrt(targetFill / fillRatio);
	}

	close() {
		this.connection.close();
		this.#announced.close();
	}
}

// An established broadcast that reloads on catalog changes.
export class Broadcast {
	watch: Watch.Broadcast;

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

	constructor(broadcast: Moq.BroadcastReader) {
		this.watch = new Watch.Broadcast(broadcast);

		this.video = new Watch.VideoRenderer();
		this.video.broadcast = this.watch.video;

		this.audio = new Watch.AudioEmitter();
		this.audio.onInit = (ctx: AudioContext, node: AudioNode) => {
			if (node.channelCount >= 2) {
				this.audioPanner = new StereoPannerNode(ctx, { channelCount: node.channelCount });
				const splitter = new ChannelSplitterNode(ctx, { channelCount: node.channelCount, numberOfOutputs: 2 });

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

		this.targetSize = Vector.create(64, 64);
		this.bounds = new Bounds(Vector.create(0, 0), this.targetSize);
	}

	close() {
		this.watch.close();
		this.video.close();
		this.audio.close();
	}
}

// Canvas setup
const canvas = document.getElementById("canvas") as HTMLCanvasElement;
canvas.width = window.innerWidth;
canvas.height = window.innerHeight;

window.addEventListener("resize", () => {
	canvas.width = window.innerWidth;
	canvas.height = window.innerHeight;
});

Moq.Connection.connect(new URL("http://localhost:4443")).then((connection) => {
	new Room(connection, "demo/", canvas);
});
