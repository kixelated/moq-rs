package warp

import (
	"bytes"
	"context"
	"encoding/binary"
	"errors"
	"fmt"
	"io"
	"io/fs"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/abema/go-mp4"
	"github.com/kixelated/invoker"
	"github.com/zencoder/go-dash/v3/mpd"
)

// This is a demo; you should actually fetch media from a live backend.
// It's just much easier to read from disk and "fake" being live.
type Media struct {
	base  fs.FS
	audio *mpd.Representation
	video *mpd.Representation
}

func NewMedia(playlistPath string) (m *Media, err error) {
	m = new(Media)

	// Create a fs.FS out of the folder holding the playlist
	m.base = os.DirFS(filepath.Dir(playlistPath))

	// Read the playlist file
	playlist, err := mpd.ReadFromFile(playlistPath)
	if err != nil {
		return nil, fmt.Errorf("failed to open playlist: %w", err)
	}

	if len(playlist.Periods) > 1 {
		return nil, fmt.Errorf("multiple periods not supported")
	}

	period := playlist.Periods[0]

	for _, adaption := range period.AdaptationSets {
		representation := adaption.Representations[0]

		if representation.MimeType == nil {
			return nil, fmt.Errorf("missing representation mime type")
		}

		switch *representation.MimeType {
		case "video/mp4":
			m.video = representation
		case "audio/mp4":
			m.audio = representation
		}
	}

	if m.video == nil {
		return nil, fmt.Errorf("no video representation found")
	}

	if m.audio == nil {
		return nil, fmt.Errorf("no audio representation found")
	}

	return m, nil
}

func (m *Media) Start() (audio *MediaStream, video *MediaStream, err error) {
	start := time.Now()

	audio, err = newMediaStream(m, m.audio, start)
	if err != nil {
		return nil, nil, err
	}

	video, err = newMediaStream(m, m.video, start)
	if err != nil {
		return nil, nil, err
	}

	return audio, video, nil
}

type MediaStream struct {
	media *Media
	init  *MediaInit

	start    time.Time
	rep      *mpd.Representation
	sequence int
}

func newMediaStream(m *Media, rep *mpd.Representation, start time.Time) (ms *MediaStream, err error) {
	ms = new(MediaStream)
	ms.media = m
	ms.rep = rep
	ms.start = start

	if rep.SegmentTemplate == nil {
		return nil, fmt.Errorf("missing segment template")
	}

	if rep.SegmentTemplate.StartNumber == nil {
		return nil, fmt.Errorf("missing start number")
	}

	ms.sequence = int(*rep.SegmentTemplate.StartNumber)

	return ms, nil
}

// Returns the init segment for the stream
func (ms *MediaStream) Init(ctx context.Context) (init *MediaInit, err error) {
	// Cache the init segment
	if ms.init != nil {
		return ms.init, nil
	}

	if ms.rep.SegmentTemplate.Initialization == nil {
		return nil, fmt.Errorf("no init template")
	}

	path := *ms.rep.SegmentTemplate.Initialization

	// TODO Support the full template engine
	path = strings.ReplaceAll(path, "$RepresentationID$", *ms.rep.ID)

	f, err := fs.ReadFile(ms.media.base, path)
	if err != nil {
		return nil, fmt.Errorf("failed to read init file: %w", err)
	}

	ms.init, err = newMediaInit(f)
	if err != nil {
		return nil, fmt.Errorf("failed to create init segment: %w", err)
	}

	return ms.init, nil
}

// Returns the next segment in the stream
func (ms *MediaStream) Segment(ctx context.Context) (segment *MediaSegment, err error) {
	if ms.rep.SegmentTemplate.Media == nil {
		return nil, fmt.Errorf("no media template")
	}

	path := *ms.rep.SegmentTemplate.Media

	// TODO Support the full template engine
	path = strings.ReplaceAll(path, "$RepresentationID$", *ms.rep.ID)
	path = strings.ReplaceAll(path, "$Number%05d$", fmt.Sprintf("%05d", ms.sequence)) // TODO TODO

	// Check if this is the first segment in the playlist
	first := ms.sequence == int(*ms.rep.SegmentTemplate.StartNumber)

	// Try openning the file
	f, err := ms.media.base.Open(path)
	if !first && errors.Is(err, os.ErrNotExist) {
		// Return EOF if the next file is missing
		return nil, nil
	} else if err != nil {
		return nil, fmt.Errorf("failed to open segment file: %w", err)
	}

	offset := ms.sequence - int(*ms.rep.SegmentTemplate.StartNumber)
	duration := time.Duration(*ms.rep.SegmentTemplate.Duration) / time.Nanosecond

	timestamp := time.Duration(offset) * duration

	// We need the init segment to properly parse the media segment
	init, err := ms.Init(ctx)
	if err != nil {
		return nil, fmt.Errorf("failed to open init file: %w", err)
	}

	segment, err = newMediaSegment(ms, init, f, timestamp)
	if err != nil {
		return nil, fmt.Errorf("failed to create segment: %w", err)
	}

	ms.sequence += 1

	return segment, nil
}

type MediaInit struct {
	Raw       []byte
	Timescale int
}

func newMediaInit(raw []byte) (mi *MediaInit, err error) {
	mi = new(MediaInit)
	mi.Raw = raw

	err = mi.parse()
	if err != nil {
		return nil, fmt.Errorf("failed to parse init segment: %w", err)
	}

	return mi, nil
}

// Parse through the init segment, literally just to populate the timescale
func (mi *MediaInit) parse() (err error) {
	r := bytes.NewReader(mi.Raw)

	_, err = mp4.ReadBoxStructure(r, func(h *mp4.ReadHandle) (interface{}, error) {
		if !h.BoxInfo.IsSupportedType() {
			return nil, nil
		}

		payload, _, err := h.ReadPayload()
		if err != nil {
			return nil, err
		}

		switch box := payload.(type) {
		case *mp4.Mdhd: // Media Header; moov -> trak -> mdia > mdhd
			if mi.Timescale != 0 {
				// verify only one track
				return nil, fmt.Errorf("multiple mdhd atoms")
			}

			mi.Timescale = int(box.Timescale)
		}

		// Expands children
		return h.Expand()
	})

	if err != nil {
		return fmt.Errorf("failed to parse MP4 file: %w", err)
	}

	return nil
}

type MediaSegment struct {
	stream    *MediaStream
	init      *MediaInit
	file      fs.File
	timestamp time.Duration
}

func newMediaSegment(s *MediaStream, init *MediaInit, file fs.File, timestamp time.Duration) (ms *MediaSegment, err error) {
	ms = new(MediaSegment)
	ms.stream = s
	ms.init = init
	ms.file = file
	ms.timestamp = timestamp
	return ms, nil
}

// Return the next atom, sleeping based on the PTS to simulate a live stream
func (ms *MediaSegment) Read(ctx context.Context) (chunk []byte, err error) {
	// Read the next top-level box
	var header [8]byte

	_, err = io.ReadFull(ms.file, header[:])
	if err != nil {
		return nil, fmt.Errorf("failed to read header: %w", err)
	}

	size := int(binary.BigEndian.Uint32(header[0:4]))
	if size < 8 {
		return nil, fmt.Errorf("box is too small")
	}

	buf := make([]byte, size)
	n := copy(buf, header[:])

	_, err = io.ReadFull(ms.file, buf[n:])
	if err != nil {
		return nil, fmt.Errorf("failed to read atom: %w", err)
	}

	sample, err := ms.parseAtom(ctx, buf)
	if err != nil {
		return nil, fmt.Errorf("failed to parse atom: %w", err)
	}

	if sample != nil {
		// Simulate a live stream by sleeping before we write this sample.
		// Figure out how much time has elapsed since the start
		elapsed := time.Since(ms.stream.start)
		delay := sample.Timestamp - elapsed

		if delay > 0 {
			// Sleep until we're supposed to see these samples
			err = invoker.Sleep(delay)(ctx)
			if err != nil {
				return nil, err
			}
		}
	}

	return buf, nil
}

// Parse through the MP4 atom, returning infomation about the next fragmented sample
func (ms *MediaSegment) parseAtom(ctx context.Context, buf []byte) (sample *mediaSample, err error) {
	r := bytes.NewReader(buf)

	_, err = mp4.ReadBoxStructure(r, func(h *mp4.ReadHandle) (interface{}, error) {
		if !h.BoxInfo.IsSupportedType() {
			return nil, nil
		}

		payload, _, err := h.ReadPayload()
		if err != nil {
			return nil, err
		}

		switch box := payload.(type) {
		case *mp4.Moof:
			sample = new(mediaSample)
		case *mp4.Tfdt: // Track Fragment Decode Timestamp; moof -> traf -> tfdt
			// TODO This box isn't required
			// TODO we want the last PTS if there are multiple samples
			var dts time.Duration
			if box.FullBox.Version == 0 {
				dts = time.Duration(box.BaseMediaDecodeTimeV0)
			} else {
				dts = time.Duration(box.BaseMediaDecodeTimeV1)
			}

			if ms.init.Timescale == 0 {
				return nil, fmt.Errorf("missing timescale")
			}

			// Convert to seconds
			// TODO What about PTS?
			sample.Timestamp = dts * time.Second / time.Duration(ms.init.Timescale)
		}

		// Expands children
		return h.Expand()
	})

	if err != nil {
		return nil, fmt.Errorf("failed to parse MP4 file: %w", err)
	}

	return sample, nil
}

func (ms *MediaSegment) Close() (err error) {
	return ms.file.Close()
}

type mediaSample struct {
	Timestamp time.Duration // The timestamp of the first sample
}
