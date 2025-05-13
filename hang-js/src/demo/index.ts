import "./index.css";
import "./highlight";

import { Watch } from "../watch";
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
const broadcastStatus = document.getElementById("broadcast-status") as HTMLSpanElement;

// If query params are provided, use them as the broadcast URL instead of the default.
const urlParams = new URLSearchParams(window.location.search);
if (urlParams.size > 0) {
	const path= urlParams.get("path") ?? "demo/bbb";
	const host = urlParams.get("host") ?? "localhost:4443";
	const scheme = urlParams.get("scheme") ?? "http";
	watch.setAttribute("url", `${scheme}://${host}/`);
	watch.setAttribute("path", path);
}

// Listen for clicks on the pause button.
pause.addEventListener("click", () => {
	watch.paused.set(!watch.paused.get());

	if (watch.paused.get()) {
		pause.textContent = "▶️";
	} else {
		pause.textContent = "⏸️";
	}
});

// Listen for clicks on the mute button.
mute.addEventListener("click", () => {
	watch.muted.set(!watch.muted.get());

	if (watch.muted.get()) {
		mute.textContent = "🔇";
		volume.value = "0";
		volumeValue.textContent = "0%";
	} else {
		mute.textContent = "🔊";
		volume.value = watch.volume.get().toString();
		volumeValue.textContent = `${(watch.volume.get() * 100).toFixed(0)}%`;
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
	const value = Number.parseFloat(volume.value);

	if (value > 0) {
		mute.textContent = "🔊";
		watch.muted.set(false);
	} else {
		mute.textContent = "🔇";
	}

	watch.volume.set(value);
	volumeValue.textContent = `${(value * 100).toFixed(0)}%`;
});

// You can "subscribe" to any of the properties.
// Or you can use SolidJS directly.
const cleanup = watch.connection.status.subscribe((value) => {
	if (value === "connected") {
		status.textContent = "🟢 Connected";
	} else if (value === "connecting") {
		status.textContent = "🟡 Connecting...";
	} else if (value === "disconnected") {
		status.textContent = "🔴 Disconnected";
	} else {
		throw new Error(`Unknown connection status: ${value}`);
	}
});

const cleanup2 = watch.broadcast.status.subscribe((value) => {
	if (value === "offline") {
		broadcastStatus.textContent = "🔴 Offline";
	} else if (value === "loading") {
		broadcastStatus.textContent = "🟡 Loading...";
	} else if (value === "live") {
		broadcastStatus.textContent = "🟢 Live";
	} else {
		throw new Error(`Unknown broadcast status: ${value}`);
	}
});

// Avoids (temporary) console warnings in development mode.
// I want to catch any memory leaks, like the above `subscribe`, because it never gets cancelled!
document.addEventListener("unload", () => {
	cleanup();
	cleanup2();
});

// Listen for changes to the latency input.
latency.addEventListener("input", () => {
	watch.latency.set(Number.parseInt(latency.value));
	latencyValue.textContent = `${latency.value}ms`;
});

// Optional: Stop downloading video when the page is hidden.
document.addEventListener("visibilitychange", () => {
	watch.paused.set(document.visibilityState === "hidden");

	if (watch.paused.get()) {
		pause.textContent = "▶️";
	} else {
		pause.textContent = "⏸️";
	}
});
