import { Queue, QueueReader, QueueWriter } from "./util/queue";

export class Group {
	readonly id: number;
	readonly writer: GroupWriter;
	readonly reader: GroupReader;

	constructor(id: number) {
		this.id = id;
		const frames = new Queue<Uint8Array>();
		this.writer = new GroupWriter(id, frames.writer);
		this.reader = new GroupReader(id, frames.reader);
	}
}

export class GroupWriter {
	readonly id: number;
	readonly frames: Watch<Uint8Array>;

	constructor(id: number, frames: QueueWriter<Uint8Array>) {
		this.id = id;
		this.frames = frames;
	}
}

export class GroupReader {
	readonly id: number;
	readonly frames: QueueReader<Uint8Array>;

	constructor(id: number, frames: QueueReader<Uint8Array>) {
		this.id = id;
		this.frames = frames;
	}

	tee(): [GroupReader, GroupReader] {
		const [one, two] = this.frames.tee();
		return [new GroupReader(this.id, one), new GroupReader(this.id, two)];
	}
}
