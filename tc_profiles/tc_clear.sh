TC='/sbin/tc'
INTERFACE_1=enp0s31f6 #$1

if [ -z $INTERFACE_1 ]; then
  echo "interface has to be specified"
  exit 1;
fi

killall tc_policy.sh 1>/dev/null 2>&1
killall sleep 1>/dev/null 2>&1
killall tc 1>/dev/null 2>&1

$TC qdisc del dev $INTERFACE_1 root handle 1:0 1>/dev/null 2>&1
$TC qdisc del dev $INTERFACE_1 root 1>/dev/null 2>&1
$TC qdisc del dev lo root 1>/dev/null 2>&1
