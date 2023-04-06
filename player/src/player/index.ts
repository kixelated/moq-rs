import Audio from "../audio"
import Transport from "../transport"
import Video from "../video"

export interface PlayerInit {
	url: string;
	canvas: HTMLCanvasElement;
}

export default class Player {
	audio: Audio;
	video: Video;
    transport: Transport;

	constructor(props: PlayerInit) {
		this.audio = new Audio()
		this.video = new Video({
			canvas: props.canvas.transferControlToOffscreen(),
		})

        this.transport = new Transport({
            url: props.url,
            audio: this.audio,
            video: this.video,
        })
	}

	async close() {
        this.transport.close()
	}

	async connect(url: string) {
        await this.transport.connect(url)
	}

	play() {
		this.audio.play({})
		//this.video.play()
	}

    onMessage(msg: any) {
        if (msg.sync) {
            msg.sync
        }
    }
}