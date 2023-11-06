#!/bin/bash
set -euo pipefail

cleanup() {
	jobs -p | xargs kill
}

trap cleanup EXIT

export RUST_LOG=debug

# echo "Starting moq-api"
PORT=4440 ./dev/api &

echo "Starting hub"
HUB_PORT="$((4440 + 1))"
export PORT=${HUB_PORT}
export API="http://localhost:4440"
export NODE="https://localhost:${PORT}"
./dev/relay &

echo "Starting relays"
for ((i = 2; i <= 5; i++)); do
	export PORT="$((4440 + i))"
	export API="http://localhost:4440"
	export NODE="https://localhost:${PORT}"
	export ARGS="--next-relays https://localhost:${HUB_PORT}"
	./dev/relay &
done

while true; do
	sleep 100
done
