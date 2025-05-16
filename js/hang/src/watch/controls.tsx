import { Match, Show, Switch, createEffect, createSignal } from "solid-js";
import { JSX } from "solid-js/jsx-runtime";
import { Watch } from "./watch";

// A simple set of controls mostly for the demo.
// You don't have to use SolidJS to implement your own controls.
export function Controls(props: { lib: Watch; root: HTMLElement }): JSX.Element {
	const lib = props.lib;
	const root = props.root;

	return (
		<div style={{ display: "flex", "justify-content": "space-between", margin: "8px 0", gap: "8px" }}>
			<Pause lib={lib} />
			<Volume lib={lib} />
			<Connection lib={lib} />
			<Broadcast lib={lib} />
			<Latency lib={lib} />
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
			<Show when={paused()} fallback={<>▶️</>}>
				⏸️
			</Show>
		</button>
	);
}

function Volume(props: { lib: Watch }): JSX.Element {
	const [volume, setVolume] = createSignal(0.5);
	const [muted, setMuted] = createSignal(false);

	const changeVolume = (str: string) => {
		const v = Number.parseFloat(str);
		if (v === 0) {
			setMuted(true);
		} else {
			setMuted(false);
			setVolume(v);
		}
	};

	const actualVolume = () => (muted() ? 0 : volume());

	createEffect(() => {
		props.lib.audio.volume.set(actualVolume());
	});

	const toggleMute = () => setMuted((m) => !m);
	const rounded = () => Math.round(actualVolume() * 100);

	return (
		<div style={{ display: "flex", "align-items": "center", gap: "0.25rem" }}>
			<button title="Mute" type="button" onClick={toggleMute}>
				<Show when={muted()} fallback={<>🔊</>}>
					🔇
				</Show>
			</button>
			<input
				type="range"
				min="0"
				max="1"
				step="0.01"
				value={actualVolume()}
				onInput={(e) => changeVolume(e.currentTarget.value)}
			/>
			<span>{rounded()}%</span>
		</div>
	);
}

function Connection(props: { lib: Watch }): JSX.Element {
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

function Latency(props: { lib: Watch }): JSX.Element {
	const setLatency = (str: string) => {
		const v = Number.parseInt(str);
		props.lib.video.latency.set(v);
		props.lib.audio.latency.set(v);
	};

	const ms = () => {
		return `${props.lib.video.latency.get()}ms`;
	};

	return (
		<div style={{ display: "flex", "align-items": "center", gap: "0.25rem" }} title="Latency">
			⏳
			<input
				type="range"
				min="0"
				max="1000"
				step="10"
				value={props.lib.video.latency.get()}
				onInput={(e) => setLatency(e.currentTarget.value)}
			/>
			<span>{ms()}</span>
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
