import { Context, MoqElement, attribute, element, jsx } from "../util";

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

import Watch from ".";

@element("moq-watch-ui")
export class WatchUi extends MoqElement {
	#inner?: Watch;
	#slot: HTMLSlotElement;
	#context?: Context;

	#controls: HTMLDivElement;
	#pause: SlButton;
	#menu: HTMLElement;
	#fullscreen: SlButton;
	#buffering: HTMLElement;
	#paused: HTMLElement;
	#status: HTMLDivElement;

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

					max-width: 100%;
					max-height: 100%;

					justify-content: center;
				}

				:host #controls {
					transform: "translate(0, 100%)",
					opacity: "0",
				}

				:host(:hover) #controls {
					transform: translate(0);
					opacity: 1;
				}
				`}
			</style>
		);

		this.#pause = (
			<sl-button>
				<sl-icon name="pause" label="Pause" />
			</sl-button>
		) as SlButton;

		this.#pause.addEventListener("click", () => this.#togglePause());

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
			if (this.#inner) {
				this.#inner.latency = targetBuffer.value;
			}
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
			<sl-button>
				<sl-icon name="fullscreen" label="Fullscreen" />
			</sl-button>
		) as SlButton;

		this.#fullscreen.addEventListener("click", () => {
			this.fullscreen = !this.fullscreen;
		});

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

		this.#slot = shadow.appendChild(<slot>No moq-watch element found.</slot>) as HTMLSlotElement;
		this.#slot.addEventListener("slotchange", () => this.#init());

		this.#status = shadow.appendChild(
			<div
				css={{
					display: "flex",
					gap: "8px",
					justifyContent: "center",
					fontFamily: "var(--sl-font-sans)",
				}}
			/>,
		) as HTMLDivElement;

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
				css={{
					position: "absolute",
					bottom: "0",
					left: "0",
					right: "0",
					justifyContent: "space-between",
					alignItems: "center",
					padding: "8px",
					gap: "8px",
					display: "flex",
					transition: "opacity 0.3s ease, transform 0.3s ease",
				}}
			>
				<div css={{ display: "flex", gap: "8px" }}>{this.#pause}</div>
				<div css={{ display: "flex", gap: "8px" }}>
					{this.#menu}
					{this.#fullscreen}
				</div>
			</div>,
		) as HTMLDivElement;
	}

	#togglePause() {
		if (!this.#inner) {
			return;
		}

		const paused = !this.#inner.paused;
		this.#inner.paused = paused;

		const icon = this.#paused.firstChild as SlIcon;
		if (paused) {
			icon.name = "play";
			icon.label = "Play";
		} else {
			icon.name = "pause";
			icon.label = "Pause";
		}
	}

	// TODO This is not unused; figure out a different more type-safe API
	private fullscreenChange(value: boolean) {
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

	#init() {
		for (const element of this.#slot.assignedElements({ flatten: true })) {
			if (element instanceof Watch) {
				this.#inner = element;
				this.#context = new Context();
				this.#run(this.#inner, this.#context);
				return;
			}
		}

		throw new Error("unable to find <moq-watch> element");
	}

	async #run(inner: Watch, context: Context) {
		await Promise.all([this.#runStatus(inner), this.#runBuffering(inner), context.done]);
	}

	async #runStatus(inner: Watch) {
		try {
			this.#status.replaceChildren(<sl-spinner />, "Initializing...");

			for await (const status of inner.connectionStatus()) {
				switch (status) {
					case "connecting":
						this.#status.replaceChildren(<sl-spinner />, "Connecting...");
						break;
					case "connected":
						this.#status.replaceChildren(<sl-spinner />, "Fetching...");
						break;
					case "disconnected":
						this.#status.replaceChildren("Disconnected");
						break;
					case "live":
						this.#status.replaceChildren();
						break;
					case "offline":
						this.#status.replaceChildren("Offline");
						break;
					default: {
						const _exhaustive: never = status;
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

	async #runBuffering(inner: Watch) {
		try {
			for await (const state of inner.rendererStatus()) {
				this.#buffering.style.display = state === "buffering" ? "flex" : "none";
				this.#paused.style.display = state === "paused" ? "flex" : "none";
			}
		} catch (err) {
			this.#buffering.style.display = "none";
			this.#paused.style.display = "none";
		}
	}
}

declare global {
	interface HTMLElementTagNameMap {
		"moq-watch-ui": WatchUi;
	}
}

export default WatchUi;
