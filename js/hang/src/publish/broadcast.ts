import * as Moq from "@kixelated/moq"
import { Derived, Signal, Signals, signal } from "@kixelated/signals"
import * as Catalog from "../catalog"
import { Connection } from "../connection"
import { Audio, AudioConstraints, AudioTrack } from "./audio"
import { Video, VideoConstraints, VideoTrack } from "./video"
import { Location, LocationProps } from "./location"

export type Device = "screen" | "camera"

export type BroadcastProps = {
	publish?: boolean
	path?: string
	audio?: AudioConstraints | boolean
	video?: VideoConstraints | boolean
	location?: LocationProps
	device?: Device

	// You can disable reloading if you want to save a round trip when you know the broadcast is already live.
	reload?: boolean
}

export class Broadcast {
	connection: Connection
	publish: Signal<boolean>
	path: Signal<string>

	audio: Audio
	video: Video
	location: Location

	catalog: Derived<Catalog.Root>
	device: Signal<Device | undefined>

	#broadcast = signal<Moq.BroadcastProducer | undefined>(undefined);
	#catalog = new Moq.TrackProducer("catalog.json", 0);
	#signals = new Signals();

	constructor(connection: Connection, props?: BroadcastProps) {
		this.connection = connection
		this.publish = signal(props?.publish ?? true)
		this.path = signal(props?.path ?? "")
		this.audio = new Audio({ constraints: props?.audio })
		this.video = new Video({ constraints: props?.video })
		this.location = new Location(props?.location)
		this.device = signal(props?.device)

		this.#signals.effect(() => {
			if (!this.publish.get()) return

			const connection = this.connection.established.get()
			if (!connection) return

			const broadcast = new Moq.BroadcastProducer(this.path.get())
			broadcast.insertTrack(this.#catalog.consume())

			this.#broadcast.set(broadcast)

			// Publish the broadcast to the connection.
			connection.publish(broadcast.consume())

			return () => {
				broadcast.close()
				this.#broadcast.set(undefined)
			}
		})

		this.#signals.effect(() => {
			const broadcast = this.#broadcast.get()
			if (!broadcast) return

			const track = this.video.track.get()
			if (!track) return

			broadcast.insertTrack(track.consume())
			return () => {
				broadcast.removeTrack(track.name)
			}
		})

		this.#signals.effect(() => {
			const broadcast = this.#broadcast.get()
			if (!broadcast) return

			const track = this.audio.track.get()
			if (!track) return

			broadcast.insertTrack(track.consume())
			return () => {
				broadcast.removeTrack(track.name)
			}
		})

		this.#signals.effect(() => {
			const broadcast = this.#broadcast.get()
			if (!broadcast) return

			const track = this.location.track

			broadcast.insertTrack(track.consume())
			return () => {
				broadcast.removeTrack(track.name)
			}
		})

		// These are separate effects because the camera audio/video constraints can be independent.
		// The screen constraints are needed at the same time.
		this.#signals.effect(() => this.#runCameraAudio())
		this.#signals.effect(() => this.#runCameraVideo())
		this.#signals.effect(() => this.#runScreen())

		this.catalog = this.#signals.derived(() => this.#runCatalog())
	}

	#runCameraAudio() {
		const device = this.device.get()
		if (device !== "camera") return

		const audio = this.audio.constraints.get()
		if (!audio) return

		const media = navigator.mediaDevices.getUserMedia({ audio })

		media
			.then((media) => {
				const track = media.getAudioTracks().at(0)
				this.audio.media.set(track as AudioTrack | undefined)
			})
			.catch((err) => {
				console.error("failed to get media", err)
			})

		return () => {
			this.audio.media.set((prev) => {
				prev?.stop()
				return undefined
			})
		}
	}

	#runCameraVideo() {
		const device = this.device.get()
		if (device !== "camera") return

		const video = this.video.constraints.get()
		if (!video) return

		const media = navigator.mediaDevices.getUserMedia({ video })

		media
			.then((media) => {
				const track = media.getVideoTracks().at(0)
				this.video.media.set(track as VideoTrack | undefined)
			})
			.catch((err) => {
				console.error("failed to get media", err)
			})

		return () => {
			this.video.media.set((prev) => {
				prev?.stop()
				return undefined
			})
		}
	}

	#runScreen() {
		const device = this.device.get()
		if (device !== "screen") return

		const audio = this.audio.constraints.get()
		const video = this.video.constraints.get()
		if (!audio && !video) return

		// TODO Expose these to the application.
		// @ts-expect-error Chrome only
		let controller: CaptureController | undefined
		// @ts-expect-error Chrome only
		if (typeof self.CaptureController !== "undefined") {
			// @ts-expect-error Chrome only
			controller = new CaptureController()
			controller.setFocusBehavior("no-focus-change")
		}

		const media = navigator.mediaDevices.getDisplayMedia({
			video,
			audio,
			// @ts-expect-error Chrome only
			controller,
			preferCurrentTab: false,
			selfBrowserSurface: "exclude",
			surfaceSwitching: "include",
			// TODO We should try to get system audio, but need to be careful about feedback.
			// systemAudio: "exclude",
		})

		media
			.then((media) => {
				const video = media.getVideoTracks().at(0) as VideoTrack | undefined
				const audio = media.getAudioTracks().at(0) as AudioTrack | undefined
				this.video.media.set(video)
				this.audio.media.set(audio)
			})
			.catch((err) => {
				console.error("failed to get media", err)
			})

		return () => {
			this.video.media.set((prev) => {
				prev?.stop()
				return undefined
			})

			this.audio.media.set((prev) => {
				prev?.stop()
				return undefined
			})
		}
	}

	#runCatalog(): Catalog.Root {
		const audio = this.audio.catalog.get()
		const video = this.video.catalog.get()
		const location = this.location.catalog.get()

		// Create the new catalog.
		const catalog = new Catalog.Root()

		// We need to wait for the encoder to fully initialize with a few frames.
		if (audio) {
			catalog.audio.push(audio)
		}

		if (video) {
			catalog.video.push(video)
		}

		if (location) {
			catalog.location = location
		}

		const encoder = new TextEncoder()
		const encoded = encoder.encode(catalog.encode())
		const catalogGroup = this.#catalog.appendGroup()
		catalogGroup.writeFrame(encoded)
		catalogGroup.close()

		return catalog
	}

	close() {
		this.#signals.close()
		this.audio.close()
		this.video.close()
	}
}
