import { type Track, decodeTrack } from "./track";

export interface Video {
	track: Track;
	codec: string;
	description?: string;
	bitrate?: number;
	frame_rate?: number;
	resolution: Dimensions;
}

export interface Dimensions {
	width: number;
	height: number;
}

export function decodeVideo(o: unknown): o is Video {
	if (typeof o !== "object" || o === null) return false;

	const obj = o as Partial<Video>;
	if (!decodeTrack(obj.track)) return false;
	if (typeof obj.codec !== "string") return false;

	obj.description = obj.description && typeof obj.description === "string" ? obj.description : undefined;

	return true;
}
