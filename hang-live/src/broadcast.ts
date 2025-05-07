import * as Watch from "@kixelated/hang/watch";
import * as Moq from "@kixelated/moq";

import { Bounds } from "./bounds";
import { Vector } from "./vector";

export class Broadcast {
	watch: Watch.Broadcast;

	audio: Watch.AudioEmitter;
	audioPanner?: StereoPannerNode;
	audioLeft?: AnalyserNode;
	audioRight?: AnalyserNode;

	video: Watch.VideoRenderer;

	bounds: Bounds;
	scale = 1.0; // 1 is 100%
	velocity = Vector.create(0, 0); // in pixels per ?

	targetPosition = Vector.create(0.5, 0.5); // in 0-1
	targetScale = 1.0; // 1 is 100%
	targetSize: Vector; // in pixels

	constructor(broadcast: Moq.BroadcastReader) {
		this.watch = new Watch.Broadcast(broadcast);

		this.video = new Watch.VideoRenderer();
		this.video.broadcast = this.watch.video;

		this.audio = new Watch.AudioEmitter();
		this.audio.onInit = (ctx: AudioContext, node: AudioNode) => {
			if (node.channelCount >= 2) {
				this.audioPanner = new StereoPannerNode(ctx, { channelCount: node.channelCount });
				const splitter = new ChannelSplitterNode(ctx, { channelCount: node.channelCount, numberOfOutputs: 2 });

				this.audioLeft = new AnalyserNode(ctx, { fftSize: 256 });
				this.audioRight = new AnalyserNode(ctx, { fftSize: 256 });

				splitter.connect(this.audioLeft, 0);
				splitter.connect(this.audioRight, 1);

				node.connect(this.audioPanner);
				this.audioPanner.connect(splitter);
				this.audioPanner.connect(ctx.destination);
			} else {
				this.audioPanner = undefined;
				this.audioLeft = new AnalyserNode(ctx, { fftSize: 256 });
				this.audioRight = this.audioLeft;

				node.connect(this.audioLeft);
				node.connect(ctx.destination); // output to the speakers
			}
		};

		this.audio.broadcast = this.watch.audio;

		this.targetSize = Vector.create(128, 128);
		this.bounds = new Bounds(Vector.create(0, 0), this.targetSize);
	}

	close() {
		this.watch.close();
		this.video.close();
		this.audio.close();
	}
}
