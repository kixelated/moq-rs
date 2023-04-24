# Put your interface name here
INTERFACE=enp0s31f6

if tc qdisc show dev $INTERFACE | grep netem; then
    sudo tc qdisc del dev $INTERFACE root
else
    echo "no netem rule"
fi
