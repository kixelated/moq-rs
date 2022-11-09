#!/bin/bash
set -euxo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"

mkcert -cert-file localhost.crt -key-file localhost.key localhost 127.0.0.1 ::1
