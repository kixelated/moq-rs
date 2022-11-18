package warp

import (
	"context"
	"crypto/tls"
	"encoding/hex"
	"fmt"
	"io"
	"log"
	"net/http"
	"os"
	"path/filepath"

	"github.com/kixelated/invoker"
	"github.com/lucas-clemente/quic-go"
	"github.com/lucas-clemente/quic-go/http3"
	"github.com/lucas-clemente/quic-go/logging"
	"github.com/lucas-clemente/quic-go/qlog"
	"github.com/marten-seemann/webtransport-go"
)

type Server struct {
	inner *webtransport.Server
	media *Media

	sessions invoker.Tasks
}

type ServerConfig struct {
	Addr   string
	Cert   *tls.Certificate
	LogDir string
}

func NewServer(config ServerConfig, media *Media) (s *Server, err error) {
	s = new(Server)

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

	tlsConfig := &tls.Config{
		Certificates: []tls.Certificate{*config.Cert},
	}

	mux := http.NewServeMux()

	s.inner = &webtransport.Server{
		H3: http3.Server{
			TLSConfig:  tlsConfig,
			QuicConfig: quicConfig,
			Addr:       config.Addr,
			Handler:    mux,
		},
		CheckOrigin: func(r *http.Request) bool { return true },
	}

	s.media = media

	mux.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		session, err := s.inner.Upgrade(w, r)
		if err != nil {
			http.Error(w, "failed to upgrade session", 500)
			return
		}

		defer session.Close()

		hijacker, ok := w.(http3.Hijacker)
		if !ok {
			log.Printf("unable to hijack connection")
			return
		}

		conn := hijacker.Connection()

		ss, err := NewSession(conn, session, s.media)
		if err != nil {
			log.Printf("failed to create session: %s", err)
			return
		}

		// Run the session in parallel, logging errors instead of crashing
		err = ss.Run(r.Context())
		if err != nil {
			log.Printf("terminated session: %s", err)
			return
		}
	})

	return s, nil
}

func (s *Server) runServe(ctx context.Context) (err error) {
	return s.inner.ListenAndServe()
}

func (s *Server) runShutdown(ctx context.Context) (err error) {
	<-ctx.Done()
	s.inner.Close()
	return ctx.Err()
}

func (s *Server) Run(ctx context.Context) (err error) {
	return invoker.Run(ctx, s.runServe, s.runShutdown, s.sessions.Repeat)
}
