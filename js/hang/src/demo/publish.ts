import "./index.css";
import "./highlight";

import { PublishElement } from "../publish/element";
import { SupportElement } from "../support/element";
export { PublishElement, SupportElement };

const publish = document.querySelector("hang-publish") as PublishElement;

// If query params are provided, use them instead of the default.
const urlParams = new URLSearchParams(window.location.search);
const name = urlParams.get("name");
if (name) {
	publish.setAttribute("name", name);
}

const watch = document.getElementById("watch") as HTMLAnchorElement;
const watchName = document.getElementById("watch-name") as HTMLSpanElement;

// An example of how you don't need to use SolidJS and can still get updates.
const cleanup = publish.lib.broadcast.name.subscribe((name) => {
	watch.href = `index.html?name=${name}`;
	watchName.textContent = name ?? "";
});

// Call the cleanup function to unsubscribe.
// Obviously this is not actually needed in this example, but it's a good practice to do so.
document.addEventListener("unload", () => {
	cleanup();
});
