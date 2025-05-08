import "./index.css";
import "./highlight";

import { Element as Publish } from "../publish";
export { Publish };

// Yes this is a terrible mess and not how we make websites in >current year<.
// But it's a demo, I'm lazy, and I don't want to require a build step.
const publish = document.getElementById("publish") as Publish;
const camera = document.getElementById("camera") as HTMLButtonElement;
const screen = document.getElementById("screen") as HTMLButtonElement;
const none = document.getElementById("none") as HTMLButtonElement;
const status = document.getElementById("status") as HTMLSpanElement;
const watch = document.getElementById("watch") as HTMLAnchorElement;

camera.addEventListener("click", () => {
	publish.device = "camera";
});
screen.addEventListener("click", () => {
	publish.device = "screen";
});
none.addEventListener("click", () => {
	publish.device = undefined;
});

// If query params are provided, use them instead of the default.
const urlParams = new URLSearchParams(window.location.search);
if (urlParams.size > 0) {
	const broadcast = urlParams.get("broadcast") ?? "demo/me";
	const host = urlParams.get("host") ?? "localhost:4443";
	const scheme = urlParams.get("scheme") ?? "http";

	publish.setAttribute("url", `${scheme}://${host}/`);
	publish.setAttribute("name", broadcast);

	watch.setAttribute(
		"href",
		`index.html?name=${broadcast}&host=${host}&scheme=${scheme}`,
	);
} else {
	watch.setAttribute("href", `index.html?name=demo/me`);
}

// Listen for connection status changes.
publish.addEventListener("moq-connection", (event) => {
	if (event.detail === "connected") {
		status.textContent = "🟢 Connected";
	} else if (event.detail === "connecting") {
		status.textContent = "🟡 Connecting...";
	} else if (event.detail === "disconnected") {
		status.textContent = "🔴 Disconnected";
	} else if (event.detail === "live") {
		status.textContent = "🟢 Live";
	} else {
		throw new Error(`Unknown connection status: ${event.detail}`);
	}
});
