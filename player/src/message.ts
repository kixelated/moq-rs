export interface Message {
	init?: MessageInit
	segment?: MessageSegment
	ping?: MessagePing
	pong?: MessagePong
}

export interface MessageInit {
	id: string
}

export interface MessageSegment {
	init: string // id of the init segment
	timestamp: number // presentation timestamp in milliseconds of the first sample
	etp: number // estimated throughput in Kbps / CTA 5006
	tc_rate: number // applied tc netem rate in Mbps
	at: number // availability time / CTA 5006
	// TODO track would be nice
}

export interface MessagePing {

}


export interface MessagePong {

}

// user preference
export interface MessagePref {
	name: string;
	value: string;
}

export interface Debug {
	max_bitrate: number
}
