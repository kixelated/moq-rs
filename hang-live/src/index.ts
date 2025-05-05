import * as Moq from "@kixelated/moq";
//import * as Hang from "@kixelated/hang";
import * as Media from "@kixelated/hang/media";

import { Vector } from "./util/vector";
import { Bounds } from "./util/bounds";

// biome-ignore lint/style/useNodejsImportProtocol: browser polyfill
import { Buffer } from "buffer";

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
				const broadcast = new Broadcast(this, this.connection.consume(update.broadcast));
				this.#broadcasts.set(update.broadcast, broadcast);
			} else {
				const broadcast = this.#broadcasts.get(update.broadcast);
				if (!broadcast) continue;

				broadcast.close();
				this.#broadcasts.delete(update.broadcast);
			}

			this.#updateScale();
		}
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
		}

		this.#render(now);

		requestAnimationFrame(this.#tick.bind(this));
	}

	#render(now: DOMHighResTimeStamp) {
		this.#ctx.clearRect(0, 0, this.#ctx.canvas.width, this.#ctx.canvas.height);

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

	#renderBroadcast(now: DOMHighResTimeStamp, broadcast: Broadcast) {
		const bounds = broadcast.bounds;

		this.#ctx.save();
		this.#ctx.font = "12px sans-serif";
		this.#ctx.translate(bounds.position.x, bounds.position.y);
		this.#ctx.fillStyle = "#000";

		const frame = broadcast.video?.frame(now);
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

		if (this.#hovering === broadcast) {
			this.#ctx.lineWidth = 1;
			this.#ctx.strokeStyle = "white";
			this.#ctx.strokeRect(0, 0, bounds.size.x, bounds.size.y);
		}

		this.#ctx.lineWidth = 3;
		this.#ctx.strokeStyle = "black";
		this.#ctx.strokeText(broadcast.broadcast.path, 4, 14);

		this.#ctx.fillStyle = "white";
		this.#ctx.fillText(broadcast.broadcast.path, 4, 14);
		this.#ctx.restore();

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
			this.#ctx.fillStyle = "#f00";
			this.#ctx.fill();
			*/
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
	room: Room;
	broadcast: Moq.BroadcastReader;

	#catalog: Moq.TrackReader;
	video?: Video;

	bounds: Bounds;
	scale = 1.0; // 1 is 100%
	velocity = Vector.create(0, 0); // in pixels per ?

	targetPosition: Vector; // in 0-1
	targetScale = 1.0; // 1 is 100%
	targetSize: Vector; // in pixels

	constructor(room: Room, broadcast: Moq.BroadcastReader) {
		this.room = room;
		this.broadcast = broadcast;

		this.#catalog = broadcast.subscribe("catalog.json", 0);

		this.targetSize = Vector.create(64, 64);

		// Generates a random value from 0 to 1 skewed towards 0.5
		const startPos = () => (Math.random() - 0.5) * Math.random() + 0.5;
		this.targetPosition = Vector.create(startPos(), startPos());

		// Follow the unit vector of the target position and go outside the screen.
		const position = Vector.create(this.targetPosition.x - 0.5, this.targetPosition.y - 0.5)
			.normalize()
			.mult(2 * Math.sqrt(this.room.canvas.width ** 2 + this.room.canvas.height ** 2));

		this.bounds = new Bounds(position, this.targetSize);

		this.#run().finally(() => this.close());
	}

	async #run() {
		for (;;) {
			const catalog = await Media.Catalog.fetch(this.#catalog);
			if (!catalog) break;

			console.debug("updated catalog", catalog);

			const video = catalog.video.at(0);
			if (video === this.video?.info) {
				// No change.
			} else {
				this.video?.close();

				if (video) {
					this.video = new Video(this.broadcast.clone(), video);
					this.targetSize = Vector.create(video.resolution.width, video.resolution.height);
				} else {
					this.video = undefined;
				}
			}
		}
	}

	close() {
		this.broadcast.close();
		this.#catalog.close();
	}
}

export class Video {
	#active?: VideoTrack;
	#visible = true;

	// Save these variables so we can reload the video if `visible` or `canvas` changes.
	broadcast: Moq.BroadcastReader;
	info: Media.Video;

	constructor(broadcast: Moq.BroadcastReader, info: Media.Video) {
		this.broadcast = broadcast;
		this.info = info;

		// TODO Perform this at a higher level
		document.addEventListener("visibilitychange", () => {
			this.#visible = document.visibilityState === "visible";
			this.#reload();
		});

		this.#visible = document.visibilityState === "visible";
		this.#reload();
	}

	frame(now: DOMHighResTimeStamp): VideoFrame | undefined {
		return this.#active?.frame(now);
	}

	#reload() {
		this.#active?.close();
		this.#active = undefined;

		if (!this.#visible) {
			return;
		}

		const sub = this.broadcast.subscribe(this.info.track.name, this.info.track.priority);
		this.#active = new VideoTrack(sub, this.info);
	}

	close() {
		this.broadcast.close();
		this.#active?.close();
	}
}

export class VideoTrack {
	info: Media.Video;

	// The maximum latency in microseconds.
	// The larger the value, the more tolerance we have for network jitter.
	// We keep at least 2 frames buffered so we can choose between them to make it smoother.
	// The default is 17ms because it's right in the middle of these two frames at 30fps.
	#maxLatency = 17_000;

	#frames: VideoFrame[] = [];

	// The difference between the wall clock units and the timestamp units, in microseconds.
	#ref?: number;

	#container: Media.Reader;
	#decoder: VideoDecoder;

	constructor(track: Moq.TrackReader, info: Media.Video) {
		this.info = info;

		this.#decoder = new VideoDecoder({
			output: (frame) => this.#decoded(frame as VideoFrame),
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

		this.#container = new Media.Reader(track);

		this.#run().finally(() => this.close());
	}

	#decoded(frame: VideoFrame) {
		this.#frames.push(frame);
		this.#prune();
	}

	frame(nowMs: DOMHighResTimeStamp): VideoFrame | undefined {
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

	async #run() {
		for (;;) {
			const frame = await this.#container.readFrame();
			if (!frame) break;

			const chunk = new EncodedVideoChunk({
				type: frame.keyframe ? "key" : "delta",
				data: frame.data,
				timestamp: frame.timestamp,
			});

			this.#decoder.decode(chunk);
		}
	}

	close() {
		this.#container.close();

		for (const frame of this.#frames) {
			frame.close();
		}

		this.#frames = [];

		try {
			this.#decoder.close();
		} catch (_) {
			// ignore
		}
	}

	get maxLatency(): DOMHighResTimeStamp {
		return this.#maxLatency / 1000;
	}

	set maxLatency(latency: DOMHighResTimeStamp) {
		this.#maxLatency = latency * 1000;
		this.#prune();
	}

	#prune() {
		while (
			this.#frames.length > 1 &&
			this.#frames[this.#frames.length - 1].timestamp - this.#frames[0].timestamp > this.#maxLatency
		) {
			const frame = this.#frames.shift();
			frame?.close();
		}
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
