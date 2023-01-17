package warp

import (
	"context"
	"encoding/binary"
	"encoding/json"
	"fmt"
	"sync"

	"github.com/kixelated/webtransport-go"
)

// Wrapper around quic.SendStream to make Write non-blocking.
// Otherwise we can't write to multiple concurrent streams in the same goroutine.
type Stream struct {
	inner webtransport.SendStream

	chunks [][]byte
	closed bool
	err    error

	notify chan struct{}
	mutex  sync.Mutex
}

func NewStream(inner webtransport.SendStream) (s *Stream) {
	s = new(Stream)
	s.inner = inner
	s.notify = make(chan struct{})
	return s
}

func (s *Stream) Run(ctx context.Context) (err error) {
	defer func() {
		s.mutex.Lock()
		s.err = err
		s.mutex.Unlock()
	}()

	for {
		s.mutex.Lock()

		chunks := s.chunks
		notify := s.notify
		closed := s.closed

		s.chunks = s.chunks[len(s.chunks):]
		s.mutex.Unlock()

		for _, chunk := range chunks {
			_, err = s.inner.Write(chunk)
			if err != nil {
				return err
			}
		}

		if closed {
			return s.inner.Close()
		}

		if len(chunks) == 0 {
			select {
			case <-ctx.Done():
				return ctx.Err()
			case <-notify:
			}
		}
	}
}

func (s *Stream) Write(buf []byte) (n int, err error) {
	s.mutex.Lock()
	defer s.mutex.Unlock()

	if s.err != nil {
		return 0, s.err
	}

	if s.closed {
		return 0, fmt.Errorf("closed")
	}

	// Make a copy of the buffer so it's long lived
	buf = append([]byte{}, buf...)
	s.chunks = append(s.chunks, buf)

	// Wake up the writer
	close(s.notify)
	s.notify = make(chan struct{})

	return len(buf), nil
}

func (s *Stream) WriteMessage(msg Message) (err error) {
	payload, err := json.Marshal(msg)
	if err != nil {
		return fmt.Errorf("failed to marshal message: %w", err)
	}

	var size [4]byte
	binary.BigEndian.PutUint32(size[:], uint32(len(payload)+8))

	_, err = s.Write(size[:])
	if err != nil {
		return fmt.Errorf("failed to write size: %w", err)
	}

	_, err = s.Write([]byte("warp"))
	if err != nil {
		return fmt.Errorf("failed to write atom header: %w", err)
	}

	_, err = s.Write(payload)
	if err != nil {
		return fmt.Errorf("failed to write payload: %w", err)
	}

	return nil
}

func (s *Stream) WriteCancel(code webtransport.StreamErrorCode) {
	s.inner.CancelWrite(code)
}

func (s *Stream) SetPriority(prio int) {
	s.inner.SetPriority(prio)
}

func (s *Stream) Close() (err error) {
	s.mutex.Lock()
	defer s.mutex.Unlock()

	if s.err != nil {
		return s.err
	}

	s.closed = true

	// Wake up the writer
	close(s.notify)
	s.notify = make(chan struct{})

	return nil
}
