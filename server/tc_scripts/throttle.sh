# Put your interface name here
INTERFACE=enp0s31f6
RATE_MBIT=$1

if [[ -z $RATE_MBIT ]]; then
    echo "missing rate"
    exit 1
fi

DELAY_MS=40
BUF_PKTS=33
BDP_BYTES=$(echo "($DELAY_MS/1000.0)*($RATE_MBIT*1000000.0/8.0)" | bc -q -l)
BDP_PKTS=$(echo "$BDP_BYTES/1500" | bc -q)
LIMIT_PKTS=$(echo "$BDP_PKTS+$BUF_PKTS" | bc -q)

echo "tc qdisc replace dev $INTERFACE root netem delay ${DELAY_MS}ms rate ${RATE_MBIT}Mbit limit ${LIMIT_PKTS}"
sudo tc qdisc replace dev $INTERFACE root netem delay ${DELAY_MS}ms rate ${RATE_MBIT}Mbit limit ${LIMIT_PKTS}