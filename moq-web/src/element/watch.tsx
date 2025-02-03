import * as Rust from "@rust";
import * as Comlink from "comlink";
import type { Bridge } from "../watch/bridge";

import { Element, attribute, element } from "./component";
import { jsx } from "./jsx";

import "@shoelace-style/shoelace/dist/components/button/button.js";
import "@shoelace-style/shoelace/dist/components/icon/icon.js";
import "@shoelace-style/shoelace/dist/components/icon-button/icon-button.js";
import "@shoelace-style/shoelace/dist/components/dropdown/dropdown.js";
import "@shoelace-style/shoelace/dist/components/divider/divider.js";
import "@shoelace-style/shoelace/dist/components/menu/menu.js";
import "@shoelace-style/shoelace/dist/components/menu-item/menu-item.js";
import "@shoelace-style/shoelace/dist/components/range/range.js";
import "@shoelace-style/shoelace/dist/components/badge/badge.js";
import "@shoelace-style/shoelace/dist/components/spinner/spinner.js";

import type SlButton from "@shoelace-style/shoelace/dist/components/button/button.js";
import type SlIcon from "@shoelace-style/shoelace/dist/components/icon/icon.js";
import type SlRange from "@shoelace-style/shoelace/dist/components/range/range.js";

export type { BackendState };
type BackendState = Rust.BackendState;

// Create a new worker instance that is shared between all instances of the Watch class.
// We wait until the worker is fully initialized before we return the proxy.
const worker: Promise<Comlink.Remote<Bridge>> = new Promise((resolve) => {
	const worker = new Worker(new URL("../watch/bridge", import.meta.url), {
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

@element("moq-watch")
export class MoqWatchElement extends Element {
	#watch: Promise<Comlink.Remote<Rust.Watch>>;

	#canvas: HTMLCanvasElement;
	#offscreenCanvas: OffscreenCanvas;

	// Optional controls
	#controls: HTMLDivElement;
	#pause: SlButton;
	#menu: HTMLElement;
	#fullscreen: SlButton;
	#buffering: HTMLElement;
	#paused: HTMLElement;

	// Optional status dialog
	#status: HTMLDivElement;

	@attribute
	accessor url = "";

	@attribute
	accessor paused = false;

	@attribute
	accessor volume = 1;

	@attribute
	accessor controls = false;

	@attribute
	accessor status = false;

	@attribute
	accessor fullscreen = false;

	// The target latency in ms.
	// A higher value means more stable playback.
	@attribute
	accessor latency = 0;

	constructor() {
		super();

		const style = (
			<style>
				{`
				:host {
					display: block;
					position: relative;
					overflow: hidden;

					max-width: 100%;
					max-height: 100%;

					justify-content: center;
				}

				:host([status]) #status {
					display: flex;
					gap: 8px;
					justify-content: center;
					font-family: var(--sl-font-sans);
				}

				:host(:not([status])) #status  {
					display: none;
				}

				:host([controls]) #controls {
					display: flex;
					transform: translate(0, 100%);
					transition: opacity 0.3s ease, transform 0.3s ease;
					opacity: 0;
				}

				:host(:not([controls])) #controls {
					display: none;
				}

				:host(:hover) #controls {
					transform: translate(0);
					opacity: 1;
				}

				`}
			</style>
		);

		this.#pause = (
			<sl-button
				onclick={() => {
					this.paused = !this.paused;
				}}
			>
				<sl-icon name="pause" label="Pause" />
			</sl-button>
		) as SlButton;

		const targetBuffer = (
			<sl-range
				label="Target Latency"
				min={0}
				max={4000}
				step={100}
				tooltipFormatter={(value) => `${(value / 1000).toFixed(1)}s`}
			/>
		) as SlRange;

		targetBuffer.addEventListener("sl-change", () => {
			this.#watch.then((watch) => watch.set_latency(targetBuffer.value));
		});

		this.#menu = (
			<sl-tooltip content="Settings">
				<sl-dropdown placement="top-start" distance={2} hoist>
					<sl-button slot="trigger">
						<sl-icon name="gear" label="Settings" />
					</sl-button>

					<sl-menu>
						<sl-menu-item>
							Latency
							<sl-menu slot="submenu">
								<sl-menu-item>{targetBuffer}</sl-menu-item>
							</sl-menu>
						</sl-menu-item>
					</sl-menu>
				</sl-dropdown>
			</sl-tooltip>
		);

		this.#fullscreen = (
			<sl-button
				onclick={() => {
					this.fullscreen = !this.fullscreen;
				}}
			>
				<sl-icon name="fullscreen" label="Fullscreen" />
			</sl-button>
		) as SlButton;

		this.#buffering = (
			<div
				css={{
					padding: "16px",
					background: "rgba(0, 0, 0, 0.7)",
					borderRadius: "8px",
					display: "none",
				}}
			>
				<sl-spinner css={{ fontSize: "2em" }} />
			</div>
		);

		this.#paused = (
			<div
				css={{
					padding: "16px",
					background: "rgba(0, 0, 0, 0.7)",
					borderRadius: "8px",
					display: "none",
				}}
			>
				<sl-icon name="pause" css={{ fontSize: "2em" }} />
			</div>
		);

		const shadow = this.attachShadow({ mode: "open" });
		shadow.appendChild(style);

		this.#status = shadow.appendChild(<div id="status" />) as HTMLDivElement;
		this.#canvas = shadow.appendChild(
			<canvas width={0} height={0} css={{ maxWidth: "100%", height: "auto" }} />,
		) as HTMLCanvasElement;

		shadow.appendChild(
			<div
				css={{
					position: "absolute",
					top: "0",
					left: "0",
					right: "0",
					bottom: "0",
					alignItems: "center",
					justifyContent: "center",
					display: "flex",
				}}
			>
				{this.#buffering}
				{this.#paused}
			</div>,
		);

		this.#controls = shadow.appendChild(
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
				<div css={{ display: "flex", gap: "8px" }}>{this.#pause}</div>
				<div css={{ display: "flex", gap: "8px" }}>
					{this.#menu}
					{this.#fullscreen}
				</div>
			</div>,
		) as HTMLDivElement;

		this.#offscreenCanvas = this.#canvas.transferControlToOffscreen();

		this.#watch = worker.then((api) => api.watch());
		this.#watch.then((watch) => {
			watch.set_canvas(Comlink.transfer(this.#offscreenCanvas, [this.#offscreenCanvas]));
			watch.set_latency(this.latency);
		});

		this.#runBackendStatus();
		this.#runBuffering();
	}

	urlChange(value: string) {
		this.#watch.then((watch) => watch.set_url(value));
	}

	pausedChange(value: boolean) {
		this.#watch.then((watch) => watch.set_paused(value));

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
		this.#watch.then((watch) => watch.set_volume(value));
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

	async #runBackendStatus() {
		try {
			this.#status.replaceChildren(<sl-spinner />, "Loading WASM Worker...");
			const watch = await this.#watch;

			const status = await Comlink.proxy(watch.status());
			while (true) {
				const next = await status.backend_state();
				if (next === undefined) {
					return;
				}

				switch (next) {
					case Rust.BackendState.Idle:
						this.#status.replaceChildren();
						break;
					case Rust.BackendState.Connecting:
						this.#status.replaceChildren(<sl-spinner />, "Connecting to Server...");
						break;
					case Rust.BackendState.Connected:
						this.#status.replaceChildren(<sl-spinner />, "Fetching Broadcast...");
						break;
					case Rust.BackendState.Live:
						this.#status.replaceChildren();
						break;
					case Rust.BackendState.Offline:
						this.#status.replaceChildren("Offline");
						break;
					default: {
						const _exhaustive: never = next;
						throw new Error(`Unhandled state: ${_exhaustive}`);
					}
				}
			}
		} catch (err) {
			this.#status.replaceChildren(
				<sl-alert variant="danger" open css={{ width: "100%" }}>
					<sl-icon slot="icon" name="exclamation-octagon" />
					<strong>Error</strong>
					<br />
					{err}
				</sl-alert>,
			);
		}
	}

	async #runBuffering() {
		try {
			const watch = await this.#watch;
			const status = await watch.status();

			for (;;) {
				const state = await status.render_state();
				this.#buffering.style.display = state === Rust.RenderState.Buffering ? "flex" : "none";
				this.#paused.style.display = state === Rust.RenderState.Paused ? "flex" : "none";
			}
		} catch (err) {
			this.#buffering.style.display = "none";
			this.#paused.style.display = "none";
		}
	}

	async latencyChange(value: number) {
		if (value < 0) {
			throw new RangeError("latency must be greater than 0");
		}
		this.#watch.then((watch) => watch.set_latency(value));
	}
}

declare global {
	interface HTMLElementTagNameMap {
		"moq-watch": MoqWatchElement;
	}
}

export default MoqWatchElement;
