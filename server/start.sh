source .env
if [[ -z $SERVER_LISTEN_ADDRESS ]]; then
    SERVER_LISTEN_ADDRESS=0.0.0.0:8443
fi

if [[ ! -d ./logs ]]; then
    mkdir logs
fi

echo "Starting server on $SERVER_LISTEN_ADDRESS"
sudo sysctl -w net.core.rmem_max=2500000 && /usr/local/go/bin/go run main.go -log-dir ./logs -addr $SERVER_LISTEN_ADDRESS