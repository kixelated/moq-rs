import { WatchElement } from "..";
export { WatchElement };

document.addEventListener(
	"DOMContentLoaded",
	() => {
		const watch = document.getElementById("watch") as WatchElement;
		const pause = document.getElementById("pause") as HTMLButtonElement;
		const fullscreen = document.getElementById("fullscreen") as HTMLButtonElement;
		const latency = document.getElementById("latency") as HTMLInputElement;
		const spinner = document.getElementById("spinner") as HTMLElement;
		const status = document.getElementById("status") as HTMLSpanElement;

		watch.addEventListener("hang-connection", (event) => {
			if (event.detail === "Connected") {
				status.textContent = "Connected";
				spinner.style.display = "none";
			} else if (event.detail === "Connecting") {
				status.textContent = "Connecting...";
				spinner.style.display = "block";
			} else if (event.detail === "Disconnected") {
				status.textContent = "Disconnected";
				spinner.style.display = "none";
			} else if (event.detail === "Live") {
				status.textContent = "";
				spinner.style.display = "none";
			} else if (event.detail === "Offline") {
				status.textContent = "Offline";
				spinner.style.display = "none";
			} else if ("Error" in event.detail) {
				status.textContent = event.detail.Error;
				spinner.style.display = "none";
			} else {
				const _: never = event.detail; // Typescript enforces that we handle all cases.
				throw new Error(`Unknown connection status: ${event.detail}`);
			}
		});

		pause?.addEventListener("click", () => {
			watch.lib.paused = !watch.lib.paused;
		});

		let fullscreenActive = false;

		fullscreen?.addEventListener("click", () => {
			if (fullscreenActive) {
				document.exitFullscreen();
			} else {
				document.body.requestFullscreen();
			}

			fullscreenActive = !fullscreenActive;
		});

		latency?.addEventListener("sl-input", () => {
			// @ts-ignore No shoelace typing for this demo.
			watch.lib.latency = Number.parseInt(latency.value);
		});
	},
	false,
);
