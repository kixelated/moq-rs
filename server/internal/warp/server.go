package warp

import (
	"context"
	"encoding/hex"
	"fmt"
	"io"
	"log"
	"net/http"
	"os"
	"path/filepath"

	"github.com/adriancable/webtransport-go"
	"github.com/kixelated/invoker"
	"github.com/lucas-clemente/quic-go"
	"github.com/lucas-clemente/quic-go/logging"
	"github.com/lucas-clemente/quic-go/qlog"
)

type Server struct {
	inner  *webtransport.Server
	media  *Media
	socket *Socket

	sessions invoker.Tasks
}

type ServerConfig struct {
	Addr     string
	CertFile string
	KeyFile  string
	LogDir   string
}

func NewServer(config ServerConfig, media *Media) (s *Server, err error) {
	s = new(Server)

	// Listen using a custom socket that simulates congestion.
	s.socket, err = NewSocket(config.Addr)
	if err != nil {
		return nil, fmt.Errorf("failed to create socket: %w", err)
	}

	quicConfig := &quic.Config{}

	if config.LogDir != "" {
		quicConfig.Tracer = qlog.NewTracer(func(p logging.Perspective, connectionID []byte) io.WriteCloser {
			path := fmt.Sprintf("%s-%s.qlog", p, hex.EncodeToString(connectionID))

			f, err := os.Create(filepath.Join(config.LogDir, path))
			if err != nil {
				// lame
				panic(err)
			}

			return f
		})
	}

	s.inner = &webtransport.Server{
		Listen:     s.socket,
		TLSCert:    webtransport.CertFile{Path: config.CertFile},
		TLSKey:     webtransport.CertFile{Path: config.KeyFile},
		QuicConfig: quicConfig,
	}

	s.media = media

	http.HandleFunc("/", func(rw http.ResponseWriter, r *http.Request) {
		session, ok := r.Body.(*webtransport.Session)
		if !ok {
			log.Print("http requests not supported")
			return
		}

		ss, err := NewSession(session, s.media, s.socket)
		if err != nil {
			// TODO handle better?
			log.Printf("failed to create warp session: %v", err)
			return
		}

		// Run the session in parallel, logging errors instead of crashing
		s.sessions.Add(func(ctx context.Context) (err error) {
			err = ss.Run(ctx)
			if err != nil {
				log.Printf("terminated session: %s", err)
			}

			return nil
		})
	})

	return s, nil
}

func (s *Server) Run(ctx context.Context) (err error) {
	return invoker.Run(ctx, s.inner.Run, s.socket.Run, s.sessions.Repeat)
}
