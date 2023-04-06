export interface Init {
	id: string
}

export interface Segment {
	init: string // id of the init segment
	timestamp: number // presentation timestamp in milliseconds of the first sample
	// TODO track would be nice
}

export interface Debug {
	max_bitrate: number
}