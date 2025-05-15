import { jsx } from "./jsx";
import { Room } from "./room";

import { Me } from "./me";
import { Connection } from "@kixelated/hang/connection"

const RELAY = new URL("http://localhost:4443");

export class HangLive extends HTMLElement {
	constructor() {
		super();

		const canvas = (
			<canvas
				css={{ display: "block", backgroundColor: "#000", width: "100%", height: "100%" }}
			/>
		) as HTMLCanvasElement;
		const connection = new Connection({ url: RELAY });

		const me = new Me({ connection, room: "demo", name: "me" });
		const room = new Room({ connection, path: "demo", canvas });

		// Register any window/document level events.
		const resize = () => {
			canvas.width = window.devicePixelRatio * window.innerWidth;
			canvas.height = window.devicePixelRatio * window.innerHeight;
		};

		const visible = () => {
			room.visible = document.visibilityState !== "hidden";
		};

		resize();
		visible();

		window.addEventListener("resize", resize);
		document.addEventListener("visibilitychange", visible);

		const shadow = this.attachShadow({ mode: "open" });
		shadow.appendChild(canvas);
		shadow.appendChild(me);
	}
}

customElements.define("hang-live", HangLive);
