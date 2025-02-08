// TODO investigate if using enums causes us to load the WASM module?
// That wouldn't happen if we could use `import type`
import * as Rust from "@rust";

export type ConnectionStatus = "disconnected" | "connecting" | "connected" | "offline" | "live";

// A helper to convert between Rust and Typescript
export function convertConnectionStatus(value: Rust.ConnectionStatus): ConnectionStatus {
	switch (value) {
		case Rust.ConnectionStatus.Disconnected:
			return "disconnected";
		case Rust.ConnectionStatus.Connecting:
			return "connecting";
		case Rust.ConnectionStatus.Connected:
			return "connected";
		case Rust.ConnectionStatus.Live:
			return "live";
		case Rust.ConnectionStatus.Offline:
			return "offline";
		default: {
			const _exhaustive: never = value;
			throw new Error(_exhaustive);
		}
	}
}
