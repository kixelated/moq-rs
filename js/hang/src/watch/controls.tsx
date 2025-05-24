import { Match, Show, Switch } from "solid-js";
import { JSX } from "solid-js/jsx-runtime";
import { AudioEmitter } from "./audio";
import { Broadcast } from "./broadcast";
import { VideoRenderer } from "./video";

// A simple set of controls mostly for the demo.
// You don't have to use SolidJS to implement your own controls; use the .subscribe() API instead.
export function Controls(props: {
	broadcast: Broadcast;
	video: VideoRenderer;
	audio: AudioEmitter;
	root: HTMLElement;
}): JSX.Element {
	const root = props.root;

	return (
		<div
			style={{
				display: "flex",
				"justify-content": "space-around",
				margin: "8px 0",
				gap: "8px",
				"align-content": "center",
			}}
		>
			<Pause video={props.video} />
			<Volume audio={props.audio} />
			<Status broadcast={props.broadcast} />
			<Fullscreen root={root} />
		</div>
	);
}

function Pause(props: { video: VideoRenderer }): JSX.Element {
	const togglePause = (e: MouseEvent) => {
		e.preventDefault();
		props.video.paused.set((prev) => !prev);
	};

	return (
		<button title="Pause" type="button" onClick={togglePause}>
			<Show when={props.video.paused.get()} fallback={<>⏸️</>}>
				▶️
			</Show>
		</button>
	);
}

function Volume(props: { audio: AudioEmitter }): JSX.Element {
	const volume = props.audio.volume;
	const muted = props.audio.muted;

	const changeVolume = (str: string) => {
		const v = Number.parseFloat(str) / 100;
		volume.set(v);
	};

	const toggleMute = () => {
		muted.set((p) => !p);
	};

	const rounded = () => Math.round(volume.get() * 100);

	return (
		<div style={{ display: "flex", "align-items": "center", gap: "0.25rem" }}>
			<button title="Mute" type="button" onClick={toggleMute}>
				<Show when={volume.get() === 0} fallback={<>🔊</>}>
					🔇
				</Show>
			</button>
			<input
				type="range"
				min="0"
				max="100"
				value={volume.get() * 100}
				onInput={(e) => changeVolume(e.currentTarget.value)}
			/>
			<span style={{ display: "inline-block", width: "2em", "text-align": "right" }}>{rounded()}%</span>
		</div>
	);
}

function Status(props: { broadcast: Broadcast }): JSX.Element {
	const url = props.broadcast.connection.url.get;
	const connection = props.broadcast.connection.status.get;
	const broadcast = props.broadcast.status.get;

	return (
		<div>
			<Switch>
				<Match when={!url()}>🔴&nbsp;No URL</Match>
				<Match when={connection() === "disconnected"}>🔴&nbsp;Disconnected</Match>
				<Match when={connection() === "connecting"}>🟡&nbsp;Connecting...</Match>
				<Match when={broadcast() === "offline"}>🔴&nbsp;Offline</Match>
				<Match when={broadcast() === "loading"}>🟡&nbsp;Loading...</Match>
				<Match when={broadcast() === "live"}>🟢&nbsp;Live</Match>
				<Match when={connection() === "connected"}>🟢&nbsp;Connected</Match>
			</Switch>
		</div>
	);
}

function Fullscreen(props: { root: HTMLElement }): JSX.Element {
	const toggleFullscreen = () => {
		if (document.fullscreenElement) {
			document.exitFullscreen();
		} else {
			props.root.requestFullscreen();
		}
	};
	return (
		<button title="Fullscreen" type="button" onClick={toggleFullscreen}>
			⛶
		</button>
	);
}
