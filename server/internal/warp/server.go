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
	"github.com/kixelated/quic-go"
	"github.com/kixelated/quic-go/http3"
	"github.com/kixelated/quic-go/logging"
	"github.com/kixelated/quic-go/qlog"
	"github.com/kixelated/webtransport-go"
)

type Server struct {
	inner    *webtransport.Server
	media    *Media
	sessions invoker.Tasks
	cert     *tls.Certificate
}

type Config struct {
	Addr   string
	Cert   *tls.Certificate
	LogDir string
	Media  *Media
}

func New(config Config) (s *Server, err error) {
	s = new(Server)
	s.cert = config.Cert
	s.media = config.Media

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
		Certificates: []tls.Certificate{*s.cert},
	}

	// Host a HTTP/3 server to serve the WebTransport endpoint
	mux := http.NewServeMux()
	mux.HandleFunc("/watch", s.handleWatch)

	s.inner = &webtransport.Server{
		H3: http3.Server{
			TLSConfig:  tlsConfig,
			QuicConfig: quicConfig,
			Addr:       config.Addr,
			Handler:    mux,
		},
		CheckOrigin: func(r *http.Request) bool { return true },
	}

	return s, nil
}

func (s *Server) runServe(ctx context.Context) (err error) {
	return s.inner.ListenAndServe()
}

func (s *Server) runShutdown(ctx context.Context) (err error) {
	<-ctx.Done()
	s.inner.Close() // close on context shutdown
	return ctx.Err()
}

func (s *Server) Run(ctx context.Context) (err error) {
	return invoker.Run(ctx, s.runServe, s.runShutdown, s.sessions.Repeat)
}

func (s *Server) handleWatch(w http.ResponseWriter, r *http.Request) {
	hijacker, ok := w.(http3.Hijacker)
	if !ok {
		panic("unable to hijack connection: must use kixelated/quic-go")
	}

	conn := hijacker.Connection()

	sess, err := s.inner.Upgrade(w, r)
	if err != nil {
		http.Error(w, "failed to upgrade session", 500)
		return
	}

	err = s.serveSession(r.Context(), conn, sess)
	if err != nil {
		log.Println(err)
	}
}

func (s *Server) serveSession(ctx context.Context, conn quic.Connection, sess *webtransport.Session) (err error) {
	defer func() {
		if err != nil {
			sess.CloseWithError(1, err.Error())
		} else {
			sess.CloseWithError(0, "end of broadcast")
		}
	}()

	ss, err := NewSession(conn, sess, s.media)
	if err != nil {
		return fmt.Errorf("failed to create session: %w", err)
	}

	err = ss.Run(ctx)
	if err != nil {
		return fmt.Errorf("terminated session: %w", err)
	}

	return nil
}
