import { Connection } from "@kixelated/hang/connection";

import { Broadcast } from "./broadcast";
import { Vector } from "./vector";

import { signal, Signal, Signals } from "@kixelated/hang/signals";
import * as Watch from "@kixelated/hang/watch";

const PADDING = 64;

export type RoomProps = {
	connection: Connection;
	canvas: HTMLCanvasElement;
	path?: string;
};

export class Room {
	// The connection to the server.
	// This is reactive; it may still be pending.
	connection: Connection;

	// All of the broadcasts keyed by their path.
	// We use the insertion order to determine the z-index.
	#broadcasts = new Map<string, Broadcast>();

	canvas: HTMLCanvasElement;
	path: Signal<string | undefined>;

	#ctx: CanvasRenderingContext2D;

	#hovering?: Broadcast;
	#dragging?: Broadcast;
	#scale = 1.0;

	#muted = true;
	#visible = true;

	#signals = new Signals();

	constructor(props: RoomProps) {
		this.connection = props.connection;
		this.canvas = props.canvas;
		this.path = signal(props.path);

		const ctx = this.canvas.getContext("2d");
		if (!ctx) {
			throw new Error("Failed to get canvas context");
		}

		this.#ctx = ctx;

		this.canvas.addEventListener("resize", () => {});

		this.canvas.addEventListener("mousedown", (e) => {
			const rect = this.canvas.getBoundingClientRect();
			const mouse = Vector.create(e.clientX - rect.left, e.clientY - rect.top);

			this.#dragging = this.#broadcastAt(mouse);
			if (!this.#dragging) return;

			// Reinsert to update the z-index.
			const name = this.#dragging.watch.path.peek();
			if (name) {
				this.#broadcasts.delete(name);
				this.#broadcasts.set(name, this.#dragging);
			}

			this.canvas.style.cursor = "grabbing";
		});

		this.canvas.addEventListener("mousemove", (e) => {
			const rect = this.canvas.getBoundingClientRect();
			const mouse = Vector.create(e.clientX - rect.left, e.clientY - rect.top);

			if (this.#dragging) {
				this.#dragging.targetPosition = Vector.create(
					mouse.x / this.canvas.width,
					mouse.y / this.canvas.height,
				);
			} else {
				this.#hovering = this.#broadcastAt(mouse);
				if (this.#hovering) {
					this.canvas.style.cursor = "grab";
				} else {
					this.canvas.style.cursor = "default";
				}
			}
		});

		this.canvas.addEventListener("mouseup", () => {
			if (this.#dragging) {
				this.#dragging = undefined;
				this.#hovering = undefined;
				this.canvas.style.cursor = "default";
			}
		});

		this.canvas.addEventListener("mouseleave", () => {
			if (this.#dragging) {
				this.#dragging = undefined;
				this.#hovering = undefined;
				this.canvas.style.cursor = "default";
			}
		});

		// We require user interaction to unmute the audio.
		this.canvas.addEventListener(
			"click",
			() => {
				this.#muted = false;
				for (const broadcast of this.#broadcasts.values()) {
					broadcast.audio.enabled.set(!this.#muted);
				}
			},
			{ once: true },
		);

		this.canvas.addEventListener(
			"wheel",
			(e) => {
				e.preventDefault(); // Prevent scroll

				let broadcast = this.#dragging;
				if (!broadcast) {
					const rect = this.canvas.getBoundingClientRect();
					const mouse = Vector.create(e.clientX - rect.left, e.clientY - rect.top);

					broadcast = this.#broadcastAt(mouse);
					if (!broadcast) return;

					this.#hovering = broadcast;
				}

				const scale = e.deltaY * 0.001;
				if (scale < 0) {
					this.canvas.style.cursor = "zoom-out";
				} else if (scale > 0) {
					this.canvas.style.cursor = "zoom-in";
				}

				broadcast.targetScale = Math.max(Math.min(broadcast.targetScale + scale, 4), 0.25);
			},
			{ passive: false },
		);

		requestAnimationFrame(this.#tick.bind(this));

		this.#signals.effect(() => this.#init());
	}

	#init() {
		const connection = this.connection.established.get();
		if (!connection) return;

		const path = this.path.get();
		if (!path) return;

		const announced = connection.announced(`${path}/`);

		(async () => {
			for (;;) {
				const update = await announced.next();

				// We're donezo.
				if (!update) break;

				const name = update.broadcast.slice(path.length + 1);

				if (update.active) {
					this.#startBroadcast(path, name);
				} else {
					this.#stopBroadcast(name);
				}
			}

			for (const broadcast of this.#broadcasts.values()) {
				broadcast.close();
			}

			this.#broadcasts.clear();
		})();

		return () => {
			announced.close();
		};
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

	#startBroadcast(path: string, name: string) {
		const targetPosition = Vector.create(Math.random(), Math.random());

		const offset = Vector.create(targetPosition.x - 0.5, targetPosition.y - 0.5)
			.normalize()
			.mult(Math.sqrt(this.canvas.width ** 2 + this.canvas.height ** 2));

		// Follow the unit vector of the target position and go outside the screen.
		const startPosition = Vector.create(
			targetPosition.x * this.canvas.width,
			targetPosition.y * this.canvas.height,
		).add(offset);

		const watch = new Watch.Broadcast({ connection: this.connection, path: `${path}/${name}`, reload: false });

		const broadcast = new Broadcast(watch, name);
		broadcast.targetPosition = targetPosition;
		broadcast.bounds.position = startPosition;

		broadcast.audio.enabled.set(!this.#muted);
		broadcast.watch.video.enabled.set(this.#visible);

		// This should never happen, but just in case.
		const old = this.#broadcasts.get(name);
		if (old) {
			old.close();
		}

		this.#broadcasts.set(name, broadcast);
	}

	#stopBroadcast(path: string) {
		const broadcast = this.#broadcasts.get(path);

		// TODO Fix the relay so it doesn't do this.
		if (!broadcast) return; //throw new Error(`Broadcast not found: ${path}`);

		broadcast.close();
		this.#broadcasts.delete(path);
	}

	#tick(now: DOMHighResTimeStamp) {
		this.#updateScale();

		const broadcasts = Array.from(this.#broadcasts.values());

		for (const broadcast of broadcasts) {
			broadcast.scale += (broadcast.targetScale - broadcast.scale) * 0.1;
			const targetSize = broadcast.targetSize.mult(broadcast.scale * this.#scale);

			// Slowly move from the actual size to the target size
			broadcast.bounds.size.x += (targetSize.x - broadcast.bounds.size.x) * 0.1;
			broadcast.bounds.size.y += (targetSize.y - broadcast.bounds.size.y) * 0.1;

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

			// Bounce off the edges of the canvas.
			/*
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
				*/
		}

		// Loop over again, this time checking for collisions.
		for (let i = 0; i < broadcasts.length; i++) {
			const a = broadcasts[i];

			for (let j = i + 1; j < broadcasts.length; j++) {
				const b = broadcasts[j];

				// Compute the intersection rectangle.
				const intersection = a.bounds.intersects(b.bounds);
				if (!intersection) {
					continue;
				}

				// Repel each other based on the size of the intersection.
				const strength = (2 * intersection.area()) / (a.bounds.area() + b.bounds.area());
				let force = a.bounds.middle().sub(b.bounds.middle()).mult(strength);

				if (this.#dragging !== a && this.#dragging !== b) {
					force = force.mult(10);
				}

				a.velocity = a.velocity.add(force);
				b.velocity = b.velocity.sub(force);
			}

			const above = PADDING - a.bounds.position.y;
			const below = a.bounds.position.y + a.bounds.size.y - (this.canvas.height - PADDING);
			const left = PADDING - a.bounds.position.x;
			const right = a.bounds.position.x + a.bounds.size.x - (this.canvas.width - PADDING);

			if (above > 0) {
				if (below > 0) {
					// Do nothing, this element is huge.
				} else {
					a.velocity.y += above;
				}
			} else if (below > 0) {
				a.velocity.y -= below;
			}

			if (left > 0) {
				if (right > 0) {
					// Do nothing, this element is huge.
				} else {
					a.velocity.x += left;
				}
			} else if (right > 0) {
				a.velocity.x -= right;
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
			this.#ctx.save();
			broadcast.renderAudio(this.#ctx, now);
			this.#ctx.restore();
		}

		for (const broadcast of this.#broadcasts.values()) {
			if (this.#dragging !== broadcast) {
				this.#ctx.save();
				broadcast.renderVideo(this.#ctx, now, {
					hovering: this.#hovering === broadcast,
				});
				this.#ctx.restore();
			}
		}

		// Render the dragging broadcast last so it's on top.
		if (this.#dragging) {
			this.#ctx.save();
			this.#ctx.fillStyle = "rgba(0, 0, 0, 0.5)";
			this.#dragging.renderVideo(this.#ctx, now, { dragging: true });
			this.#ctx.restore();
		}
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
		this.#signals.close();

		for (const broadcast of this.#broadcasts.values()) {
			broadcast.close();
		}
	}

	get visible() {
		return this.#visible;
	}

	set visible(visible: boolean) {
		this.#visible = visible;

		for (const broadcast of this.#broadcasts.values()) {
			broadcast.watch.video.enabled.set(this.#visible);
		}
	}
}
