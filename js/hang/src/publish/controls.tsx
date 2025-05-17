import { Match, Switch, createSelector } from "solid-js";
import { JSX } from "solid-js/jsx-runtime";
import { Device } from "./broadcast";
import { Publish } from "./publish";

export function PublishControls(props: { lib: Publish }): JSX.Element {
	return (
		<div
			style={{
				display: "flex",
				"justify-content": "space-around",
				gap: "16px",
				margin: "8px 0",
				"align-content": "center",
			}}
		>
			<Select lib={props.lib} />
			<Connection lib={props.lib} />
			<Publishing lib={props.lib} />
		</div>
	);
}

function Connection(props: { lib: Publish }): JSX.Element {
	const status = props.lib.connection.status.get;

	return (
		<div>
			<Switch>
				<Match when={status() === "connected"}>🟢&nbsp;Connected</Match>
				<Match when={status() === "connecting"}>🟡&nbsp;Connecting...</Match>
				<Match when={status() === "disconnected"}>🔴&nbsp;Disconnected</Match>
			</Switch>
		</div>
	);
}

function Publishing(props: { lib: Publish }): JSX.Element {
	const broadcast = props.lib.broadcast.path.get;
	const audio = props.lib.broadcast.audio.catalog.get;
	const video = props.lib.broadcast.video.catalog.get;

	return (
		<div>
			<Switch>
				<Match when={!broadcast()}>🔴&nbsp;Choose Name</Match>
				<Match when={audio() && video()}>🟢&nbsp;Live</Match>
				<Match when={audio() && !video()}>🟡&nbsp;Audio Only</Match>
				<Match when={!audio() && video()}>🟡&nbsp;Video Only</Match>
				<Match when={!audio() && !video()}>🔴&nbsp;Select Device</Match>
			</Switch>
		</div>
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
			<button
				id="camera"
				title="Camera"
				type="button"
				onClick={() => setDevice("camera")}
				style={buttonStyle("camera")}
			>
				🎥
			</button>
			<button
				id="screen"
				title="Screen"
				type="button"
				onClick={() => setDevice("screen")}
				style={buttonStyle("screen")}
			>
				🖥️
			</button>
			<button
				id="none"
				title="Nothing"
				type="button"
				onClick={() => setDevice(undefined)}
				style={buttonStyle(undefined)}
			>
				🚫
			</button>
		</div>
	);
}
