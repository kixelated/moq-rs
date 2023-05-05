import Transport from "../transport"
import Media from "../media"

export interface PlayerInit {
	url: string;
	fingerprint?: WebTransportHash; // the certificate fingerprint, temporarily needed for local development
	canvas: HTMLCanvasElement;
}

export default class Player {
	media: Media;
    transport: Transport;

	constructor(props: PlayerInit) {
		this.media = new Media({
			canvas: props.canvas.transferControlToOffscreen(),
		})

        this.transport = new Transport({
            url: props.url,
			fingerprint: props.fingerprint,
            media: this.media,
        })
	}

	async close() {
        this.transport.close()
	}

	play() {
		//this.media.play()
	}
}