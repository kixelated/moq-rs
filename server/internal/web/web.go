package web

import (
	"context"
	"log"
	"net/http"
	"time"

	"github.com/kixelated/invoker"
)

type Server struct {
	inner  http.Server
	config Config
}

type Config struct {
	Addr        string
	CertFile    string
	KeyFile     string
	Fingerprint string // the TLS certificate fingerprint
}

func New(config Config) (s *Server) {
	s = new(Server)
	s.config = config

	s.inner = http.Server{
		Addr: config.Addr,
	}

	http.HandleFunc("/fingerprint", s.handleFingerprint)

	return s
}

func (s *Server) Run(ctx context.Context) (err error) {
	return invoker.Run(ctx, s.runServe, s.runShutdown)
}

func (s *Server) runServe(context.Context) (err error) {
	// NOTE: Doesn't support context, which is why we need runShutdown
	err = s.inner.ListenAndServeTLS(s.config.CertFile, s.config.KeyFile)
	log.Println(err)
	return err
}

// Gracefully shut down the server when the context is cancelled
func (s *Server) runShutdown(ctx context.Context) (err error) {
	<-ctx.Done()

	timeout, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	_ = s.inner.Shutdown(timeout)

	return ctx.Err()
}

// Return the sha256 of the certificate as a temporary work-around for local development.
// TODO remove this when WebTransport uses the system CA
func (s *Server) handleFingerprint(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Access-Control-Allow-Origin", "*")
	_, _ = w.Write([]byte(s.config.Fingerprint))
}
