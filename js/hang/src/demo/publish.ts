import "./index.css";
import "./highlight";

import { PublishElement } from "../publish/element";
import { SupportElement } from "../support/element";
export { PublishElement, SupportElement };

const publish = document.querySelector("hang-publish") as PublishElement;
const watch = document.getElementById("watch") as HTMLAnchorElement;
const watchName = document.getElementById("watch-name") as HTMLSpanElement;

// If query params are provided, use them instead of the default.
const urlParams = new URLSearchParams(window.location.search);
const name = urlParams.get("name");
if (name) {
	publish.setAttribute("url", `https://localhost:4443/demo/${name}.hang`);
	watch.href = `index.html?name=${name}`;
	watchName.textContent = name;
}
