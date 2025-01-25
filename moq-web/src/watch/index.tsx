import type * as Rust from "@rust";
import * as Comlink from "comlink";
import type { Bridge } from "./bridge";

import { Element, attribute, element } from "../element/component";
import { jsx } from "../element/jsx";

import "@shoelace-style/shoelace/dist/components/button/button.js";
import "@shoelace-style/shoelace/dist/components/icon/icon.js";

import type SlButton from "@shoelace-style/shoelace/dist/components/button/button.js";
import type SlIcon from "@shoelace-style/shoelace/dist/components/icon/icon.js";

export type { WatchState };
type WatchState = Rust.WatchState;

// Create a new worker instance that is shared between all instances of the Watch class.
// We wait until the worker is fully initialized before we return the proxy.
const worker: Promise<Comlink.Remote<Bridge>> = new Promise((resolve) => {
	const worker = new Worker(new URL("./bridge", import.meta.url), {
		type: "module",
	});
	worker.addEventListener(
		"message",
		(event) => {
			const proxy: Comlink.Remote<Bridge> = Comlink.wrap(worker);
			resolve(proxy);
		},
		{ once: true },
	);
});

/*
	async *state(): AsyncGenerator<WatchState> {
		const watch = await this.#watch;
		const states = await Comlink.proxy(watch.states());

		try {
			while (true) {
				const next = await states.next();
				if (!next) {
					return;
				}

				yield next;
			}
		} finally {
			states.free();
		}
	}
		*/

@element("moq-watch")
export class MoqWatch extends Element {
	#watch: Promise<Comlink.Remote<Rust.Watch>>;
	#canvas: OffscreenCanvas;
	#controls: HTMLDivElement;

	#pause: SlButton;
	#fullscreen: SlButton;

	@attribute
	accessor url = "";

	@attribute
	accessor paused = false;

	@attribute
	accessor volume = 1;

	@attribute
	accessor controls = false;

	@attribute
	accessor fullscreen = false;

	constructor() {
		super();

		const style = (
			<style>
				{`
				:host {
					display: block;
					position: relative;
					overflow: hidden;
				}

				:host(:not([controls])) #controls {
					display: none;
				}

				:host([controls]) #controls {
					display: flex;
					transform: translate(0, 100%);
					transition: opacity 0.3s ease, transform 0.3s ease;
					opacity: 0;
				}

				:host(:hover) #controls {
					transform: translate(0);
					opacity: 1;
				}

				`}
			</style>
		);

		const canvas = (
			<canvas
				css={{ display: "block", maxWidth: "100%", height: "auto" }}
				width={0}
				height={0}
			/>
		) as HTMLCanvasElement;

		this.#pause = (
			<sl-button
				css={{ background: "black", borderRadius: "8px" }}
				onclick={() => {
					this.paused = !this.paused;
				}}
			>
				<sl-icon name="pause" label="Pause" />
			</sl-button>
		) as SlButton;

		this.#fullscreen = (
			<sl-button
				onclick={() => {
					this.fullscreen = !this.fullscreen;
				}}
			>
				<sl-icon name="fullscreen" label="Fullscreen" />
			</sl-button>
		) as SlButton;

		this.#controls = (
			<div
				id="controls"
				css={{
					position: "absolute",
					bottom: "0",
					left: "0",
					right: "0",
					justifyContent: "space-between",
					alignItems: "center",
					padding: "8px",
					gap: "8px",
				}}
			>
				{this.#pause}
				{this.#fullscreen}
			</div>
		) as HTMLDivElement;

		const shadow = this.attachShadow({ mode: "open" });
		shadow.appendChild(style);
		shadow.appendChild(canvas);
		shadow.appendChild(this.#controls);

		this.#canvas = canvas.transferControlToOffscreen();

		this.#watch = worker.then((api) => api.watch());
		this.#watch.then((watch) =>
			watch.canvas(Comlink.transfer(this.#canvas, [this.#canvas])),
		);
	}

	urlChange(value: string) {
		this.#watch.then((watch) => watch.url(value));
	}

	pausedChange(value: boolean) {
		this.#watch.then((watch) => watch.paused(value));

		const icon = this.#pause.firstChild as SlIcon;
		if (value) {
			icon.name = "play";
			icon.label = "Play";
		} else {
			icon.name = "pause";
			icon.label = "Pause";
		}
	}

	volumeChange(value: number) {
		if (value < 0 || value > 1) {
			throw new RangeError("volume must be between 0 and 1");
		}
		this.#watch.then((watch) => watch.volume(value));
	}

	async fullscreenChange(value: boolean) {
		const icon = this.#fullscreen.firstChild as SlIcon;
		if (value) {
			icon.name = "fullscreen-exit";
			icon.label = "Exit Fullscreen";
			this.requestFullscreen().catch(() => {
				this.fullscreen = false;
			});
		} else {
			icon.name = "fullscreen";
			icon.label = "Fullscreen";
			document.exitFullscreen().catch(() => {});
		}
	}
}

declare global {
	interface HTMLElementTagNameMap {
		"moq-watch": MoqWatch;
	}
}

export default MoqWatch;
