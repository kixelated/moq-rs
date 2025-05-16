import "./index.css";
import "./highlight";

import { WatchElement } from "../watch";
export { WatchElement };

const watch = document.querySelector("hang-watch") as WatchElement;

// If query params are provided, use it as the broadcast name.
const urlParams = new URLSearchParams(window.location.search);
const broadcast = urlParams.get("broadcast");
if (broadcast) {
	console.log("Setting broadcast to", broadcast);
	watch.setAttribute("broadcast", broadcast);
}