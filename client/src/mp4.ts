// Wrapper around MP4Box to play nicely with MP4Box.
// I tried getting a mp4box.all.d.ts file to work but just couldn't figure it out
import { createFile, ISOFile, DataStream, BoxParser } from "./mp4box.all"

// Rename some stuff so it's on brand.
export { createFile as MP4New, ISOFile as MP4File, DataStream as MP4Stream, BoxParser as MP4Parser }

export type MP4ArrayBuffer = ArrayBuffer & {fileStart: number};

export interface MP4MediaTrack {
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

export interface MP4VideoData {
	width: number;
	height: number;
}

export interface MP4VideoTrack extends MP4MediaTrack {
	video: MP4VideoData;
}

export interface MP4AudioData {
	sample_rate: number;
	channel_count: number;
	sample_size: number;
}

export interface MP4AudioTrack extends MP4MediaTrack {
	audio: MP4AudioData;
}

export type MP4Track = MP4VideoTrack | MP4AudioTrack;

export interface MP4Info {
	duration: number;
	timescale: number;
	fragment_duration: number;
	isFragmented: boolean;
	isProgressive: boolean;
	hasIOD: boolean;
	brands: string[];
	created: Date;
	modified: Date;
	tracks: MP4Track[];
	mime: string;
	videoTracks: MP4Track[];
	audioTracks: MP4Track[];
}

export interface MP4Sample {
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
