package warp

import (
	"context"
	"fmt"
	"math/rand"
	"net"
	"sync"
	"syscall"
	"time"

	"github.com/kixelated/invoker"
)

// Perform network simulation in-process to make a simpler demo.
// You should not use this in production; there are much better ways to throttle a network.
type Socket struct {
	inner *net.UDPConn

	writeRate int   // bytes per second
	writeErr  error // return this error on all future writes

	writeQueue     []packet // packets ready to be sent
	writeQueueSize int      // number of bytes in the queue
	writeQueueMax  int      // number of bytes allowed in the queue

	writeLastTime time.Time
	writeLastSize int

	writeLoss float64 // packet loss percentage

	writeNotify chan struct{} // closed when rate or queue is changed
	writeMutex  sync.Mutex
}

type packet struct {
	Addr net.Addr
	Data []byte
}

func NewSocket(addr string) (s *Socket, err error) {
	s = new(Socket)

	uaddr, err := net.ResolveUDPAddr("udp", addr)
	if err != nil {
		return nil, fmt.Errorf("failed to resolve addr: %w", err)
	}

	s.inner, err = net.ListenUDP("udp", uaddr)
	if err != nil {
		return nil, fmt.Errorf("failed to listen: %w", err)
	}

	s.writeNotify = make(chan struct{})

	return s, nil
}

func (s *Socket) ReadFrom(p []byte) (n int, addr net.Addr, err error) {
	// TODO throttle reads?
	return s.inner.ReadFrom(p)
}

// Queue up packets to be sent
func (s *Socket) WriteTo(p []byte, addr net.Addr) (n int, err error) {
	s.writeMutex.Lock()
	defer s.writeMutex.Unlock()

	if s.writeErr != nil {
		return 0, s.writeErr
	}

	if s.writeQueueMax > 0 && s.writeQueueSize+len(p) > s.writeQueueMax {
		// Gotta drop the packet
		return len(p), nil
	}

	if len(s.writeQueue) == 0 && s.writeRate == 0 {
		// If there's no queue and no throttling, write directly
		if s.writeLoss == 0 || rand.Float64() >= s.writeLoss {
			_, err = s.inner.WriteTo(p, addr)
			if err != nil {
				s.writeErr = err
				return 0, err
			}
		}

		return len(p), nil
	}

	// Make a copy of the packet
	pc := packet{
		Addr: addr,
		Data: append([]byte{}, p...),
	}

	if len(s.writeQueue) == 0 {
		// Wakeup the writer goroutine.
		close(s.writeNotify)
		s.writeNotify = make(chan struct{})
	}

	s.writeQueue = append(s.writeQueue, pc)
	s.writeQueueSize += len(p)

	return len(p), nil
}

// Perform the writing in another goroutine.
func (s *Socket) runWrite(ctx context.Context) (err error) {
	timer := time.NewTimer(time.Second)
	timer.Stop()

	s.writeMutex.Lock()
	defer s.writeMutex.Unlock()

	for {
		// Lock is held at the start of the loop

		lastTime := s.writeLastTime
		lastSize := s.writeLastSize
		rate := s.writeRate
		notify := s.writeNotify
		ready := len(s.writeQueue) > 0

		if !ready {
			// Unlock while we wait for changes.
			s.writeMutex.Unlock()

			select {
			case <-ctx.Done():
				s.writeMutex.Lock() // gotta lock again just for the defer...
				return ctx.Err()
			case <-notify:
				// Something changed, try again
				s.writeMutex.Lock()
				continue
			}
		}

		now := time.Now()

		if lastSize > 0 && rate > 0 {
			// Compute the amount of time it should take to send lastSize bytes
			delay := time.Second * time.Duration(lastSize) / time.Duration(rate)
			next := lastTime.Add(delay)

			delay = next.Sub(now)
			if delay > 0 {
				// Unlock while we sleep.
				s.writeMutex.Unlock()

				// Reuse the timer instance
				// No need to drain the timer beforehand
				timer.Reset(delay)

				select {
				case <-ctx.Done():
					s.writeMutex.Lock() // gotta lock again just for the defer...
					return ctx.Err()
				case <-timer.C:
					now = next
					s.writeMutex.Lock()
				case <-notify:
					// Something changed, try again
					if !timer.Stop() {
						// Drain the timer
						<-timer.C
					}

					s.writeMutex.Lock()
					continue
				}
			}
		}

		// Send the first packet in the queue
		p := s.writeQueue[0]
		s.writeQueue = s.writeQueue[1:]
		s.writeQueueSize -= len(p.Data)
		s.writeLastTime = now
		s.writeLastSize = len(p.Data)

		loss := s.writeLoss

		if loss > 0 || rand.Float64() >= loss {
			_, err = s.inner.WriteTo(p.Data, p.Addr)
			if err != nil {
				s.writeErr = err
				return err
			}
		}
	}
}

// Set the number of *bytes* that can be written within a second, or -1 for unlimited.
// Defaults to unlimited.
func (s *Socket) SetWriteRate(rate int) {
	s.writeMutex.Lock()
	defer s.writeMutex.Unlock()

	s.writeRate = rate
	close(s.writeNotify)
	s.writeNotify = make(chan struct{})
}

// Set the maximum number of bytes to queue before we drop packets.
// Defaults to unlimited (!)
func (s *Socket) SetWriteBuffer(size int) {
	s.writeMutex.Lock()
	defer s.writeMutex.Unlock()

	s.writeQueueMax = size
	if s.writeQueueMax > 0 {
		// Remove from the queue until the limit has been met
		for s.writeQueueSize > s.writeQueueMax {
			last := s.writeQueue[len(s.writeQueue)-1]
			s.writeQueue = s.writeQueue[:len(s.writeQueue)-1]
			s.writeQueueSize -= len(last.Data)
		}
	}

	close(s.writeNotify)
	s.writeNotify = make(chan struct{})
}

func (s *Socket) SetWriteLoss(percent float64) {
	s.writeMutex.Lock()
	defer s.writeMutex.Unlock()

	s.writeLoss = percent
}

func (s *Socket) Close() (err error) {
	return s.inner.Close()
}

func (s *Socket) LocalAddr() net.Addr {
	return s.inner.LocalAddr()
}

func (s *Socket) SetDeadline(t time.Time) error {
	return s.inner.SetDeadline(t)
}

func (s *Socket) SetReadDeadline(t time.Time) error {
	return s.inner.SetReadDeadline(t)
}

func (s *Socket) SetReadBuffer(size int) error {
	return s.inner.SetReadBuffer(size)
}

func (s *Socket) SetWriteDeadline(t time.Time) error {
	return s.inner.SetWriteDeadline(t)
}

func (s *Socket) SyscallConn() (syscall.RawConn, error) {
	return s.inner.SyscallConn()
}

func (s *Socket) Run(ctx context.Context) (err error) {
	return invoker.Run(ctx /*s.runRead, */, s.runWrite)
}
