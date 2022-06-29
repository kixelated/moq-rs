package warp

type Message struct {
	Init     *MessageInit     `json:"init,omitempty"`
	Segment  *MessageSegment  `json:"segment,omitempty"`
	Throttle *MessageThrottle `json:"x-throttle,omitempty"`
}

type MessageInit struct {
	Id int `json:"id"` // ID of the init segment
}

type MessageSegment struct {
	Init      int `json:"init"`      // ID of the init segment to use for this segment
	Timestamp int `json:"timestamp"` // PTS of the first frame in milliseconds
}

type MessageThrottle struct {
	Rate   int     `json:"rate"`   // Artificially limit the socket byte rate per second
	Buffer int     `json:"buffer"` // Artificially limit the socket buffer to the number of bytes
	Loss   float64 `json:"loss"`   // Artificially increase packet loss percentage from 0.0 - 1.0
}
