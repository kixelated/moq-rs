import "./index.css";
import "./demo/highlight";

import { createMemo, createSelector, createSignal, onCleanup } from "solid-js";
import { For, render } from "solid-js/web";

import * as Moq from "@kixelated/moq";
import { Match } from "solid-js";
import { Show } from "solid-js";
import { JSX } from "solid-js/jsx-runtime";
import { Switch } from "solid-js/web";


const root = document.querySelector("#root") as HTMLDivElement;

function Announcements(): JSX.Element {
	// Store every announcement we've received.
	const [announces, setAnnounces] = createSignal(
		new Map<
			string,
			{
				active: boolean;
				when: number;
			}
		>(),
	);

	const [status, setStatus] = createSignal<"disconnected" | "connecting" | "connected">("disconnected");
	const isStatus = createSelector(status);

	// Trigger an update every 10 seconds for UI refresh
	const [now, setNow] = createSignal(performance.now());
	const interval = setInterval(() => setNow(performance.now()), 1_000);
	onCleanup(() => clearInterval(interval));

	(async () => {
		setStatus("connecting");
		const conn = await Moq.Connection.connect(new URL("http://localhost:4443/"));
		setStatus("connected");
		const announced = conn.announced();

		for (;;) {
			const announce = await announced.next();
			if (!announce) break;

			// Update the current time early.
			setNow(performance.now());

			setAnnounces((prev) => {
				const copy = new Map(prev);
				copy.set(announce.path, {
					active: announce.active,
					when: performance.now(),
				});
				return copy;
			});
		}
	})().finally(() => setStatus("disconnected"));

	function since(timestamp: number): string {
		const delta = Math.max(0, now() - timestamp);
		const seconds = Math.floor(delta / 1000);
		const minutes = Math.floor(delta / 60000);
		const hours = Math.floor(delta / 3600000);
		const days = Math.floor(delta / 86400000);

		if (days > 0) return `${days}d ago`;
		if (hours > 0) return `${hours}h ago`;
		if (minutes > 0) return `${minutes}m ago`;

		return `${seconds}s ago`;
	}

	const active = createMemo(() => {
		return Array.from(announces().entries())
			.filter(([_, props]) => props.active)
			.map(([broadcast, props]) => {
				return {
					name: broadcast,
					...props,
				};
			});
	});

	const inactive = createMemo(() => {
		return Array.from(announces().entries())
			.filter(([_, props]) => !props.active)
			.map(([broadcast, props]) => {
				return {
					name: broadcast,
					...props,
				};
			});
	});

	return (
		<div class="flex flex-col gap-1">
			<Switch>
				<Match when={isStatus("disconnected")}>
					<h3>🔴 Disconnected</h3>
				</Match>
				<Match when={isStatus("connecting")}>
					<h3>🟡 Connecting...</h3>
				</Match>
				<Match when={isStatus("connected")}>
					<Show when={active().length > 0}>
						<h3>🟢 Active Broadcasts:</h3>
						<ul>
							<For each={active()}>
								{(broadcast) => (
									<li>
										<Show when={broadcast.name.endsWith(".hang")} fallback={broadcast.name}>
											<a href={`index.html?name=${broadcast.name.slice(0, -5)}`}>
												{broadcast.name}
											</a>
										</Show>
										<span
											style={{
												"font-size": "0.75em",
												color: "gray",
												"font-style": "italic",
												"margin-left": "1em",
											}}
										>
											{since(broadcast.when)}
										</span>
									</li>
								)}
							</For>
						</ul>
					</Show>
					<Show when={inactive().length > 0}>
						<h3>🔴 Inactive Broadcasts:</h3>
						<ul>
							<For each={inactive()}>
								{(broadcast) => (
									<li class="flex gap-4 items-center">
										<div>{broadcast.name}</div>
										<div style={{ "font-size": "0.75em", color: "gray", "font-style": "italic" }}>
											- {since(broadcast.when)}
										</div>
									</li>
								)}
							</For>
						</ul>
					</Show>
				</Match>
			</Switch>
		</div>
	);
}

render(() => <Announcements />, root);
