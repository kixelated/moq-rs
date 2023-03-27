// Wrapper around MP4Box to play nicely with MP4Box.
// I tried getting a mp4box.all.d.ts file to work but just couldn't figure it out
import { createFile, ISOFile, DataStream, BoxParser } from "./mp4box.all"

// Rename some stuff so it's on brand.
export { createFile as New, ISOFile as File, DataStream as Stream, BoxParser as Parser }

export type ArrayBufferOffset = ArrayBuffer & {fileStart: number};

export interface MediaTrack {
	id: number;
	created: Date;
	modified: Date;
	movie_duration: number;
	layer: number;
	alternate_group: number;
	volume: number;
	track_width: number;
	track_height: number;
	timescale: number;
	duration: number;
	bitrate: number;
	codec: string;
	language: string;
	nb_samples: number;
}

export interface VideoData {
	width: number;
	height: number;
}

export interface VideoTrack extends MediaTrack {
	video: VideoData;
}

export interface MP4AudioData {
	sample_rate: number;
	channel_count: number;
	sample_size: number;
}

export interface AudioTrack extends MediaTrack {
	audio: MP4AudioData;
}

export type Track = VideoTrack | AudioTrack;

export interface Info {
	duration: number;
	timescale: number;
	fragment_duration: number;
	isFragmented: boolean;
	isProgressive: boolean;
	hasIOD: boolean;
	brands: string[];
	created: Date;
	modified: Date;
	tracks: Track[];
	mime: string;
	videoTracks: Track[];
	audioTracks: Track[];
}

export interface Sample {
	number: number;
	track_id: number;
	timescale: number;
	description_index: number;
	description: any;
	data: ArrayBuffer;
	size: number;
	alreadyRead: number;
	duration: number;
	cts: number;
	dts: number;
	is_sync: boolean;
	is_leading: number;
	depends_on: number;
	is_depended_on: number;
	has_redundancy: number;
	degration_priority: number;
	offset: number;
	subsamples: any;
}

export { Init, InitParser } from "./init"