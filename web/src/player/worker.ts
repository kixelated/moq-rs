import Renderer from "./renderer"
import Decoder from "./decoder"
import * as Message from "./message"

let decoder: Decoder
let renderer: Renderer

self.addEventListener("message", async (e: MessageEvent) => {
	if (e.data.config) {
		const config = e.data.config as Message.Config

		renderer = new Renderer(config)
		decoder = new Decoder(renderer)
	} else if (e.data.init) {
		const init = e.data.init as Message.Init
		await decoder.receiveInit(init)
	} else if (e.data.segment) {
		const segment = e.data.segment as Message.Segment
		await decoder.receiveSegment(segment)
	} else if (e.data.play) {
		const play = e.data.play as Message.Play
		await renderer.play(play)
	}
})
