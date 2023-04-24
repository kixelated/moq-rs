package warp

import (
	"context"
	"encoding/binary"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"log"
	"math"
	"time"

	"github.com/kixelated/invoker"
	"github.com/kixelated/quic-go"
	"github.com/kixelated/webtransport-go"
)

// A single WebTransport session
type Session struct {
	conn  quic.Connection
	inner *webtransport.Session

	media *Media
	inits map[string]*MediaInit
	audio *MediaStream
	video *MediaStream

	server *Server

	streams invoker.Tasks

	prefs map[string]string

	continueStreaming bool
	audioTimeOffset   time.Duration
	videoTimeOffset   time.Duration
}

func NewSession(connection quic.Connection, session *webtransport.Session, media *Media, server *Server) (s *Session, err error) {
	s = new(Session)
	s.server = server
	s.conn = connection
	s.inner = session
	s.media = media
	s.continueStreaming = true
	s.server.continueStreaming = true
	return s, nil
}

func (s *Session) Run(ctx context.Context) (err error) {
	s.inits, s.audio, s.video, err = s.media.Start(s.conn.GetMaxBandwidth)
	s.prefs = make(map[string]string)
	if err != nil {
		return fmt.Errorf("failed to start media: %w", err)
	}

	// Once we've validated the session, now we can start accessing the streams
	return invoker.Run(ctx, s.runAccept, s.runAcceptUni, s.runInit, s.runAudio, s.runVideo, s.streams.Repeat)
}

func (s *Session) runAccept(ctx context.Context) (err error) {
	for {
		stream, err := s.inner.AcceptStream(ctx)
		if err != nil {
			return fmt.Errorf("failed to accept bidirectional stream: %w", err)
		}

		// Warp doesn't utilize bidirectional streams so just close them immediately.
		// We might use them in the future so don't close the connection with an error.
		stream.CancelRead(1)
	}
}

func (s *Session) runAcceptUni(ctx context.Context) (err error) {
	for {
		stream, err := s.inner.AcceptUniStream(ctx)
		if err != nil {
			return fmt.Errorf("failed to accept unidirectional stream: %w", err)
		}

		s.streams.Add(func(ctx context.Context) (err error) {
			return s.handleStream(ctx, stream)
		})
	}
}

func (s *Session) handleStream(ctx context.Context, stream webtransport.ReceiveStream) (err error) {
	defer func() {
		if err != nil {
			stream.CancelRead(1)
		}
	}()

	var header [8]byte
	for {
		_, err = io.ReadFull(stream, header[:])
		if errors.Is(io.EOF, err) {
			return nil
		} else if err != nil {
			return fmt.Errorf("failed to read atom header: %w", err)
		}

		size := binary.BigEndian.Uint32(header[0:4])
		name := string(header[4:8])

		if size < 8 {
			return fmt.Errorf("atom size is too small")
		} else if size > 42069 { // arbitrary limit
			return fmt.Errorf("atom size is too large")
		} else if name != "warp" {
			return fmt.Errorf("only warp atoms are supported")
		}

		payload := make([]byte, size-8)

		_, err = io.ReadFull(stream, payload)
		if err != nil {
			return fmt.Errorf("failed to read atom payload: %w", err)
		}

		log.Println("received message:", string(payload))

		msg := Message{}

		err = json.Unmarshal(payload, &msg)
		if err != nil {
			return fmt.Errorf("failed to decode json payload: %w", err)
		}

		if msg.Debug != nil {
			s.setDebug(msg.Debug)
		}

		if msg.Pref != nil {
			fmt.Printf("* Pref received name: %s value: %s\n", msg.Pref.Name, msg.Pref.Value)
			s.setPref(msg.Pref)
		}

		if msg.Ping != nil {
			println("Ping received")
			err := s.sendPong(msg.Ping, ctx)
			if err != nil {
				return err
			}
		}
	}
}

func (s *Session) runInit(ctx context.Context) (err error) {
	for _, init := range s.inits {
		err = s.writeInit(ctx, init)
		if err != nil {
			return fmt.Errorf("failed to write init stream: %w", err)
		}
	}

	return nil
}

func (s *Session) runAudio(ctx context.Context) (err error) {
	start := time.Now()
	for {
		if !s.continueStreaming {
			// Sleep to let cpu off
			err := invoker.Sleep(10 * time.Millisecond)(ctx)
			if err != nil {
				return fmt.Errorf("failed in runAudio: %w", err)
			}
			s.audioTimeOffset += time.Since(start)
			continue
		} else {
			// reset start
			start = time.Now()
		}

		segment, err := s.audio.Next(ctx, s, s.audioTimeOffset)
		if err != nil {
			return fmt.Errorf("failed to get next segment: %w", err)
		}

		if segment == nil {
			return nil
		}

		err = s.writeSegment(ctx, segment)
		if err != nil {
			return fmt.Errorf("failed to write segment stream: %w", err)
		}
	}
}

func (s *Session) runVideo(ctx context.Context) (err error) {
	start := time.Now()
	for {
		if !s.continueStreaming {
			// Sleep to let cpu off
			err := invoker.Sleep(10 * time.Millisecond)(ctx)
			if err != nil {
				return fmt.Errorf("failed in runAudio: %w", err)
			}
			s.videoTimeOffset += time.Since(start)
			continue
		} else {
			// reset start
			start = time.Now()
		}

		segment, err := s.video.Next(ctx, s, s.videoTimeOffset)
		if err != nil {
			return fmt.Errorf("failed to get next segment: %w", err)
		}

		if segment == nil {
			return nil
		}

		err = s.writeSegment(ctx, segment)
		if err != nil {
			return fmt.Errorf("failed to write segment stream: %w", err)
		}
	}
}

// Create a stream for an INIT segment and write the container.
func (s *Session) writeInit(ctx context.Context, init *MediaInit) (err error) {
	temp, err := s.inner.OpenUniStreamSync(ctx)
	if err != nil {
		return fmt.Errorf("failed to create stream: %w", err)
	}

	// Wrap the stream in an object that buffers writes instead of blocking.
	stream := NewStream(temp)
	s.streams.Add(stream.Run)

	defer func() {
		if err != nil {
			stream.WriteCancel(1)
		}
	}()

	stream.SetPriority(math.MaxInt)

	err = stream.WriteMessage(Message{
		Init: &MessageInit{Id: init.ID},
	})

	if err != nil {
		return fmt.Errorf("failed to write init header: %w", err)
	}

	_, err = stream.Write(init.Raw)
	if err != nil {
		return fmt.Errorf("failed to write init data: %w", err)
	}

	return nil
}

// Create a stream for a segment and write the contents, chunk by chunk.
func (s *Session) writeSegment(ctx context.Context, segment *MediaSegment) (err error) {
	temp, err := s.inner.OpenUniStreamSync(ctx)
	if err != nil {
		return fmt.Errorf("failed to create stream: %w", err)
	}

	// Wrap the stream in an object that buffers writes instead of blocking.
	stream := NewStream(temp)
	s.streams.Add(stream.Run)

	defer func() {
		if err != nil {
			stream.WriteCancel(1)
		}
	}()

	ms := int(segment.timestamp / time.Millisecond)

	// newer segments take priority
	stream.SetPriority(ms)

	tcRate := s.server.tcRate
	if tcRate == -1 {
		tcRate = 0
	}

	init_message := Message{
		Segment: &MessageSegment{
			Init:             segment.Init.ID,
			Timestamp:        ms,
			ETP:              int(s.conn.GetMaxBandwidth() / 1024),
			TcRate:           tcRate * 1024,
			AvailabilityTime: int(time.Now().UnixMilli()),
		},
	}
	/*

			Segments on the Wire
			------------------------------------------------------
		    [chunk_S1_N] ...  [chunk_S1_1]  [segment 1 init]
			------------------------------------------------------

			Stream multiplexing in QUIC:
			-----------------------------------------------------------
		    [chunk_S1_N]  ..[chunk_S2_M] .. [chunk_S1_2]...[chunk_S1_1]
			----------------------------------------------------------

			Head of Line Blocking Problem in TCP:
			------------------------------------
			TCP Buffer
			Pipeline
			|    x   | c_s1_1 Head of line blocking
			| c_s1_2 |
			| c_s1_3 |
			| c_s2_1 |
			| c_s1_4 |

			Quic treats each stream differently
			-----------------------------------
		    Stream 1
			|    x   |
			| c_s1_2 |
			| c_s1_3 |
			| c_s1_4 |

			Stream 2
			| c_s2_1 |
			|        |

	*/

	err = stream.WriteMessage(init_message)

	if err != nil {
		return fmt.Errorf("failed to write segment header: %w", err)
	}

	segment_size := 0
	box_count := 0
	chunk_count := 0

	print_moof_sizes := false

	last_moof_size := 0

	for {
		// Get the next fragment
		start := time.Now().UnixMilli()

		buf, err := segment.Read(ctx)
		if errors.Is(err, io.EOF) {
			break
		} else if err != nil {
			return fmt.Errorf("failed to read segment data: %w", err)
		}

		segment_size += len(buf)
		box_count++

		if print_moof_sizes {
			if string(buf[4:8]) == "moof" {
				last_moof_size = len(buf)
				chunk_count++
			} else if string(buf[4:8]) == "mdat" {
				chunk_size := last_moof_size + len(buf)
				fmt.Printf("* chunk: %d size: %d time offset: %d\n", chunk_count, chunk_size, time.Now().UnixMilli()-start)
			}
		}

		// NOTE: This won't block because of our wrapper
		_, err = stream.Write(buf)
		if err != nil {
			return fmt.Errorf("failed to write segment data: %w", err)
		}

	}

	// for debug purposes
	fmt.Printf("* id: %s ts: %d etp: %d segment size: %d box count:%d chunk count: %d\n", init_message.Segment.Init, init_message.Segment.Timestamp, init_message.Segment.ETP, segment_size, box_count, chunk_count)

	err = stream.Close()
	if err != nil {
		return fmt.Errorf("failed to close segemnt stream: %w", err)
	}

	return nil
}

func (s *Session) setDebug(msg *MessageDebug) {
	if msg.MaxBitrate != nil {
		s.conn.SetMaxBandwidth(uint64(*msg.MaxBitrate))
	} else if msg.ContinueStreaming != nil {
		s.continueStreaming = *msg.ContinueStreaming
		s.server.continueStreaming = *msg.ContinueStreaming
	} else if *msg.TcReset {
		// setting tcRate to -1 is a signal to reset tc rate
		s.server.tcRate = -1
		s.server.isTcActive = false
		s.server.continueStreaming = true
	}
}

func (s *Session) setPref(msg *MessagePref) {
	s.prefs[msg.Name] = msg.Value
}

func (s *Session) sendPong(msg *MessagePing, ctx context.Context) (err error) {
	temp, err := s.inner.OpenUniStreamSync(ctx)
	if err != nil {
		return fmt.Errorf("failed to create stream: %w", err)
	}

	// Wrap the stream in an object that buffers writes instead of blocking.
	stream := NewStream(temp)
	s.streams.Add(stream.Run)

	defer func() {
		if err != nil {
			stream.WriteCancel(1)
		}
	}()

	err = stream.WriteMessage(
		Message{
			Pong: &MessagePong{},
		})
	if err != nil {
		return fmt.Errorf("failed to write init header: %w", err)
	}
	return nil
}
