import { Match, Show, Switch, createEffect, createSignal } from "solid-js";
import { JSX } from "solid-js/jsx-runtime";
import { Watch } from "./watch";

// A simple set of controls mostly for the demo.
// You don't have to use SolidJS to implement your own controls; use the .subscribe() API instead.
export function WatchControls(props: { lib: Watch; root: HTMLElement }): JSX.Element {
	const lib = props.lib;
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
			<Pause lib={lib} />
			<Volume lib={lib} />
			<Connection lib={lib} />
			<Broadcast lib={lib} />
			<Fullscreen lib={lib} root={root} />
		</div>
	);
}

function Pause(props: { lib: Watch }): JSX.Element {
	const [paused, setPaused] = createSignal(false);

	createEffect(() => {
		props.lib.audio.paused.set(paused());
		props.lib.video.paused.set(paused());
	});

	const togglePause = () => setPaused((p) => !p);
	return (
		<button title="Pause" type="button" onClick={togglePause}>
			<Show when={paused()} fallback={<>⏸️</>}>
				▶️
			</Show>
		</button>
	);
}

function Volume(props: { lib: Watch }): JSX.Element {
	const volume = props.lib.audio.volume;

	// Keep track of the last non-zero volume for the unmute button.
	let unmuteVolume = volume.peek() || 0.5;

	const changeVolume = (str: string) => {
		const v = Number.parseFloat(str);
		volume.set(v);
		if (v > 0) {
			unmuteVolume = v;
		} else {
			unmuteVolume = 0.5;
		}
	};

	const toggleMute = () => {
		if (volume.get() === 0) {
			volume.set(unmuteVolume);
		} else {
			volume.set(0);
		}
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
				max="1"
				step="0.01"
				value={volume.get()}
				onInput={(e) => changeVolume(e.currentTarget.value)}
			/>
			<span style={{ display: "inline-block", width: "2em", "text-align": "right" }}>{rounded()}%</span>
		</div>
	);
}

function Connection(props: { lib: Watch }): JSX.Element {
	const url = props.lib.connection.url.get;
	const status = props.lib.connection.status.get;

	return (
		<div>
			<Switch>
				<Match when={!url()}>🔴&nbsp;Missing URL</Match>
				<Match when={status() === "connected"}>🟢&nbsp;Connected</Match>
				<Match when={status() === "connecting"}>🟡&nbsp;Connecting...</Match>
				<Match when={status() === "disconnected"}>🔴&nbsp;Disconnected</Match>
			</Switch>
		</div>
	);
}

function Broadcast(props: { lib: Watch }): JSX.Element {
	const status = props.lib.broadcast.status.get;

	return (
		<div>
			<Switch>
				<Match when={status() === "live"}>🟢&nbsp;Live</Match>
				<Match when={status() === "loading"}>🟡&nbsp;Loading...</Match>
				<Match when={status() === "offline"}>🔴&nbsp;Offline</Match>
			</Switch>
		</div>
	);
}

function Fullscreen(props: { lib: Watch; root: HTMLElement }): JSX.Element {
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
