import "./index.css";
import "./highlight";

import { PublishElement } from "../publish/element";
import { SupportElement } from "../support/element";
export { PublishElement, SupportElement };

const publish = document.querySelector("hang-publish") as PublishElement;

// If query params are provided, use them instead of the default.
const urlParams = new URLSearchParams(window.location.search);
const broadcastName = urlParams.get("broadcast");
if (broadcastName) {
	publish.setAttribute("broadcast", broadcastName);
}

const watch = document.getElementById("watch") as HTMLAnchorElement;
const watchName = document.getElementById("watch-name") as HTMLSpanElement;

// An example of how you don't need to use SolidJS and can still get updates.
publish.lib.broadcast.path.subscribe((path) => {
	watch.href = `index.html?broadcast=${path}`;
	watchName.textContent = path ?? "";
});
