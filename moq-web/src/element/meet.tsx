import { Room, RoomAction, type RoomAnnounced } from "../room";

import type { MoqWatch } from "./watch";

import { Element, attribute, element } from "./component";
import { jsx } from "./jsx";

import "@shoelace-style/shoelace/dist/components/spinner/spinner.js";
import "@shoelace-style/shoelace/dist/components/alert/alert.js";
import "@shoelace-style/shoelace/dist/components/icon/icon.js";

@element("moq-meet")
export class MoqMeet extends Element {
	#room: Room;
	#container: HTMLDivElement;
	#broadcasts: Set<MoqWatch> = new Set();
	#status: HTMLDivElement;

	@attribute
	accessor room = "";

	@attribute
	accessor controls = false;

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
				`}
			</style>
		);

		this.#status = (<div />) as HTMLDivElement;

		this.#container = (<div css={{ display: "flex", gap: "8px", alignItems: "center" }} />) as HTMLDivElement;

		this.#room = new Room();
		const announced = this.#room.announced();
		this.#runAnnounced(announced).finally(() => announced.free());

		const shadow = this.attachShadow({ mode: "open" });
		shadow.appendChild(style);
		shadow.appendChild(this.#status);
		shadow.appendChild(this.#container);
	}

	roomChange(value: string) {
		this.#room.url = value;
	}

	controlsChange(value: boolean) {
		for (const broadcast of this.#broadcasts) {
			broadcast.controls = value;
		}
	}

	async #runAnnounced(announced: RoomAnnounced) {
		this.#status.replaceChildren(<sl-spinner />);

		let live = false;

		while (true) {
			const announce = await announced.next();
			if (!announce) {
				// TODO get error message
				this.#status.replaceChildren(
					<sl-alert variant="danger" open css={{ width: "100%" }}>
						<sl-icon slot="icon" name="exclamation-octagon" />
						<strong>Disconnected</strong>
						<br />
						TODO get the error message
					</sl-alert>,
				);
				return;
			}

			this.#status.replaceChildren();

			switch (announce.action) {
				case RoomAction.Join:
					this.#join(announce.name);
					break;
				case RoomAction.Leave:
					this.#leave(announce.name);
					break;
				case RoomAction.Live:
					live = true;
					break;
			}

			if (live && this.#broadcasts.size === 0) {
				this.#status.replaceChildren(<span css={{ fontFamily: "var(--sl-font-sans)" }}>"🦗 nobody is here 🦗"</span>);
			}
		}
	}

	#join(name: string) {
		const watch = (
			<moq-watch
				id={`broadcast-${name}`}
				url={`${this.room}/${name}`}
				controls={this.controls}
				css={{ borderRadius: "0.5rem", overflow: "hidden" }}
			/>
		) as MoqWatch;

		this.#container.appendChild(watch);
		this.#broadcasts.add(watch);
	}

	#leave(name: string) {
		const id = `#broadcast-${name}`;

		const watch = this.#container.querySelector(id) as MoqWatch | null;
		if (!watch) {
			console.warn(`Broadcast not found: ${id}`);
			return;
		}

		watch.remove();
		this.#broadcasts.delete(watch);
	}
}

declare global {
	interface HTMLElementTagNameMap {
		"moq-meet": MoqMeet;
	}
}

export default MoqMeet;
