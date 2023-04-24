package warp

type Message struct {
	Init    *MessageInit    `json:"init,omitempty"`
	Segment *MessageSegment `json:"segment,omitempty"`
	Ping    *MessagePing    `json:"x-ping,omitempty"`
	Pong    *MessagePong    `json:"pong,omitempty"`
	Debug   *MessageDebug   `json:"debug,omitempty"`
	Pref    *MessagePref    `json:"x-pref,omitempty"`
}

type MessageInit struct {
	Id string `json:"id"` // ID of the init segment
}

type MessageSegment struct {
	Init             string  `json:"init"`      // ID of the init segment to use for this segment
	Timestamp        int     `json:"timestamp"` // PTS of the first frame in milliseconds
	ETP              int     `json:"etp"`       // Estimated throughput in bytes - CTA 5006
	TcRate           float64 `json:"tc_rate"`   // Applied tc rate
	AvailabilityTime int     `json:"at"`        // The wallclock time at which the first byte of this object became available at the origin for successful request. - CTA 5006
}

type MessageDebug struct {
	MaxBitrate        *int  `json:"max_bitrate,omitempty"`        // Artificially limit the QUIC max bitrate
	ContinueStreaming *bool `json:"continue_streaming,omitempty"` // Resume or pause streaming
	TcReset           *bool `json:"tc_reset,omitempty"`           // Set tc profile
}

type MessagePing struct {
}

type MessagePong struct {
}

type MessagePref struct {
	Name  string `json:"name"`
	Value string `json:"value"`
}
