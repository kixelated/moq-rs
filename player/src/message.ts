export interface Message {
	init?: MessageInit
	segment?: MessageSegment
}

export interface MessageInit {
	id: number // integer id
}

export interface MessageSegment {
	init: number      // integer id of the init segment
	timestamp: number // presentation timestamp in milliseconds of the first sample
	// TODO track would be nice
}
