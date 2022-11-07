package main

import (
	"bufio"
	"context"
	"errors"
	"flag"
	"fmt"
	"log"
	"os"
	"strings"

	"github.com/kixelated/invoker"
	"github.com/kixelated/warp-sample/internal/warp"
)

func main() {
	err := run(context.Background())
	if err == nil {
		return
	}

	log.Println(err)

	var errPanic invoker.ErrPanic

	// TODO use an interface
	if errors.As(err, &errPanic) {
		stack := string(errPanic.Stack())

		scanner := bufio.NewScanner(strings.NewReader(stack))
		for scanner.Scan() {
			log.Println(scanner.Text())
		}
	}

	os.Exit(1)
}

func run(ctx context.Context) (err error) {
	addr := flag.String("addr", ":4443", "HTTPS server address")
	cert := flag.String("tls-cert", "../cert/localhost.warp.demo.crt", "TLS certificate file path")
	key := flag.String("tls-key", "../cert/localhost.warp.demo.key", "TLS certificate file path")
	logDir := flag.String("log-dir", "", "logs will be written to the provided directory")

	dash := flag.String("dash", "../media/fragmented.mpd", "DASH playlist path")

	flag.Parse()

	media, err := warp.NewMedia(*dash)
	if err != nil {
		return fmt.Errorf("failed to open media: %w", err)
	}

	config := warp.ServerConfig{
		Addr:     *addr,
		CertFile: *cert,
		KeyFile:  *key,
		LogDir:   *logDir,
	}

	ws, err := warp.NewServer(config, media)
	if err != nil {
		return fmt.Errorf("failed to create warp server: %w", err)
	}

	log.Printf("listening on %s", *addr)

	return invoker.Run(ctx, invoker.Interrupt, ws.Run)
}
