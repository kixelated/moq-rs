export interface Message {
	init?: MessageInit
	segment?: MessageSegment
}

export interface MessageInit {
	id: string
}

export interface MessageSegment {
	init: string // id of the init segment
	timestamp: number // presentation timestamp in milliseconds of the first sample
	// TODO track would be nice
}

export interface Debug {
	max_bitrate: number
}