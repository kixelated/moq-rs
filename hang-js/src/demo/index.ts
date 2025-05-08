import "./index.css";
import "./highlight";

import { Element as Watch } from "../watch";
export { Watch };

// Yes this is a terrible mess and not how we make websites in >current year<.
// But this is a demo and I don't feel like making it more complicated via React.
const watch = document.getElementById("watch") as Watch;
const pause = document.getElementById("pause") as HTMLButtonElement;
const fullscreen = document.getElementById("fullscreen") as HTMLButtonElement;
const status = document.getElementById("status") as HTMLSpanElement;
const volume = document.getElementById("volume") as HTMLInputElement;
const container = document.getElementById("container") as HTMLDivElement;
const latency = document.getElementById("latency") as HTMLInputElement;
const latencyValue = document.getElementById("latency-value") as HTMLSpanElement;
const volumeValue = document.getElementById("volume-value") as HTMLSpanElement;
const mute = document.getElementById("mute") as HTMLButtonElement;

// If query params are provided, use them as the broadcast URL instead of the default.
const urlParams = new URLSearchParams(window.location.search);
if (urlParams.size > 0) {
	const broadcast = urlParams.get("name") ?? "demo/bbb";
	const host = urlParams.get("host") ?? "localhost:4443";
	const scheme = urlParams.get("scheme") ?? "http";
	watch.setAttribute("url", `${scheme}://${host}`);
	watch.setAttribute("name", broadcast);
}

// Listen for clicks on the pause button.
pause.addEventListener("click", () => {
	watch.video.paused = !watch.video.paused;
	watch.audio.muted = watch.video.paused;

	if (watch.video.paused) {
		pause.textContent = "▶️";
	} else {
		pause.textContent = "⏸️";
	}
});

// Listen for clicks on the mute button.
mute.addEventListener("click", () => {
	watch.audio.muted = !watch.audio.muted;

	if (watch.audio.muted) {
		mute.textContent = "🔇";
		volume.value = "0";
		volumeValue.textContent = "0%";
	} else {
		mute.textContent = "🔊";
		volume.value = watch.audio.volume.toString();
		volumeValue.textContent = `${(watch.audio.volume * 100).toFixed(0)}%`;
	}
});

// Listen for clicks on the fullscreen button.
let fullscreenActive = false;
fullscreen.addEventListener("click", () => {
	if (fullscreenActive) {
		document.exitFullscreen();
	} else {
		container.requestFullscreen();
	}

	fullscreenActive = !fullscreenActive;
});

// Listen for changes to the volume input.
volume.addEventListener("input", () => {
	if (watch.audio.muted) {
		mute.textContent = "🔊";
		watch.audio.muted = false;
	}

	watch.audio.volume = Number.parseFloat(volume.value);
	volumeValue.textContent = `${(Number.parseFloat(volume.value) * 100).toFixed(0)}%`;

	if (Number.parseFloat(volume.value) === 0) {
		mute.textContent = "🔇";
	}
});

// Listen for connection status changes.
watch.addEventListener("moq-connection", (event) => {
	if (event.detail === "connected") {
		status.textContent = "🟢 Connected";
	} else if (event.detail === "connecting") {
		status.textContent = "🟡 Connecting...";
	} else if (event.detail === "disconnected") {
		status.textContent = "🔴 Disconnected";
	} else {
		throw new Error(`Unknown connection status: ${event.detail}`);
	}
});

// Listen for changes to the latency input.
latency.addEventListener("input", () => {
	watch.audio.latency = Number.parseInt(latency.value);
	watch.video.latency = Number.parseInt(latency.value);
	latencyValue.textContent = `${latency.value}ms`;
});

// Optional: Stop downloading video when the page is hidden.
document.addEventListener("visibilitychange", () => {
	watch.video.paused = document.visibilityState === "hidden";

	if (watch.video.paused) {
		pause.textContent = "▶️";
	} else {
		pause.textContent = "⏸️";
	}
});
