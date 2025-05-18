import "./index.css";
import "./highlight";

import { WatchElement } from "../watch/element";
import { SupportElement } from "../support/element";

export { WatchElement, SupportElement };

const watch = document.querySelector("hang-watch") as WatchElement;

// If query params are provided, use it as the broadcast name.
const urlParams = new URLSearchParams(window.location.search);
const broadcast = urlParams.get("broadcast");
if (broadcast) {
	watch.setAttribute("broadcast", broadcast);
}
