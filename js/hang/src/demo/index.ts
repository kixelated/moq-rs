import "./index.css";
import "./highlight";

import { SupportElement } from "../support/element";
import { WatchElement } from "../watch/element";

export { WatchElement, SupportElement };

const watch = document.querySelector("hang-watch") as WatchElement;

// If query params are provided, use it as the broadcast name.
const urlParams = new URLSearchParams(window.location.search);
const name = urlParams.get("name");
if (name) {
	watch.setAttribute("url", `https://localhost:4443/demo/${name}.hang`);
}
