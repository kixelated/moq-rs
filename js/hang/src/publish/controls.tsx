import { JSX } from "solid-js/jsx-runtime";
import { Publish } from "./publish";
import { Device } from "./broadcast";
import { Switch, Match, createSelector } from "solid-js";

export function Controls(props: { lib: Publish }): JSX.Element {
	return (
		<div style={{ display: "flex", "justify-content": "space-around", gap: "16px", margin: "8px 0" }}>
			<Status lib={props.lib} />
			<Select lib={props.lib} />
			<Broadcast lib={props.lib} />
		</div>
	);
}

function Status(props: { lib: Publish }): JSX.Element {
	const status = props.lib.connection.status.get;

	return (
		<Switch>
			<Match when={status() === "connected"}>🟢&nbsp;Connected</Match>
			<Match when={status() === "connecting"}>🟡&nbsp;Connecting...</Match>
			<Match when={status() === "disconnected"}>🔴&nbsp;Disconnected</Match>
		</Switch>
	);
}

function Select(props: { lib: Publish }): JSX.Element {
	const setDevice = (device: Device | undefined) => {
		props.lib.broadcast.device.set(device);
	};

	const selected = createSelector(props.lib.broadcast.device.get);

	const buttonStyle = (id: Device | undefined) => ({
		cursor: "pointer",
		opacity: selected(id) ? 1 : 0.5,
	});

	return (
		<div style={{ display: "flex", gap: "16px" }}>
			Device:
			<button id="camera" title="Camera" type="button" onClick={() => setDevice("camera")} style={buttonStyle("camera")}>
				🎥
			</button>
			<button id="screen" title="Screen" type="button" onClick={() => setDevice("screen")} style={buttonStyle("screen")}>
				🖥️
			</button>
			<button id="none" title="Nothing" type="button" onClick={() => setDevice(undefined)} style={buttonStyle(undefined)}>
				🚫
			</button>
		</div>
	);
}

function Broadcast(props: { lib: Publish }): JSX.Element {
	const setBroadcast = (broadcast: string) => {
		props.lib.broadcast.path.set(broadcast);
	};

	return (
		<div style={{ display: "flex", gap: "8px" }}>
			Broadcast:
			<input
				type="text"
				id="broadcast"
				placeholder="demo/me"
				title="Broadcast name"
				onChange={(e) => setBroadcast(e.target.value)}
			/>
		</div>
	);
}

/*
// If query params are provided, use them instead of the default.
const urlParams = new URLSearchParams(window.location.search);
const scheme = urlParams.get("scheme");
const host = urlParams.get("host");
if (host || scheme) {
	publish.setAttribute("url", `${scheme ?? "http"}://${host ?? "localhost:4443"}/`);
}

const broadcastName = urlParams.get("broadcast");
if (broadcastName) {
	publish.setAttribute("broadcast", broadcastName);
	broadcast.value = broadcastName;
}

broadcast.addEventListener("change", () => {
	publish.setAttribute("broadcast", broadcast.value);
	watch.setAttribute("href", `index.html?broadcast=${broadcast.value}`); // TODO: Add host and scheme
});

// Listen for connection status changes.
const cleanup = publish.connection.status.subscribe((value) => {
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

document.addEventListener("unload", () => cleanup());
*/
