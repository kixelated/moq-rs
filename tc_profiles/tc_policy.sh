#!/bin/bash
TC="/sbin/tc"
PORT_1=8443
INTERFACE_1=enp0s31f6 #$1
FILE_1=zafer_profile #$2

#PORT=443
#INTERFACE_2=$INTERFACE_1
#FILE_2=$FILE_1


if [ -z $INTERFACE_1 ]; then
  echo "interface has to be specified"
  exit 1;
fi

if [ -z $FILE_1 ]; then
  echo "policy file name has to be specified"
  exit 1;
fi

parsePolicyFile () {
  device=$1
  filename=$2
  classId=$3
  childClassId=$4
  if [ -z "$filename" ] || [ -z "$classId" ];then
    echo "filename and classid paramters required"
  else
    latestLoss="0%";
    latestDelay="0ms";
    while read -r line; do
      if [[ $line == \#* ]];then
        continue; 
      else
        keys=($line)
        comm=${keys[0]}
        value=${keys[1]}
	case $comm in
	  rate)
	    echo "setting rate on $device $classId $value"
      burst=`awk "BEGIN {print $value/800*1000}"`
	    $TC class change dev $device parent 1: classid 1:$classId htb rate $value burst ${burst} cburst ${burst}
	    $TC class change dev $device parent 1: classid 1:$childClassId htb rate $value burst ${burst} cburst ${burst}
      #$TC qdisc del dev $device root 1>/dev/null 2>&1
      #$TC qdisc change dev $device root tbf rate $value burst $burst latency 1ms
	    ;;
    loss)
 	    latestLoss=$value;
	    echo "setting loss on $device $classId $value"
	    $TC qdisc change dev $device parent 1:$classId netem loss $latestLoss delay $latestDelay
	    ;;
	  delay)
	    latestDelay=$value;
	    echo "setting delay on $device $classId $value"
	    $TC qdisc change dev $device parent 1:$classId netem loss $latestLoss delay $latestDelay
	    ;;
	   wait)
	    echo "waiting for $device $value seconds"
	    sleep $value
	    ;;
	esac
      fi
    done < "$filename"
  fi
}

policyLoop () {
  device=$1
  filename=$2
  classId=$3
  childClassId=$4
  while true; do
    parsePolicyFile $device $filename $classId $childClassId
  done
}

currentIfNo=1
while [[ -v INTERFACE_$currentIfNo ]]; do
  interface=INTERFACE_$currentIfNo 
  interface="${!interface}"
  $TC qdisc del dev $interface root 1>/dev/null 2>&1
  $TC qdisc add dev $interface root handle 1: htb default 10
  ((currentIfNo++))
done 

currentIfNo=1
while [[ -v PORT_$currentIfNo ]]; do
  interface=INTERFACE_$currentIfNo 
  interface="${!interface}"
  port=PORT_$currentIfNo 
  port="${!port}"
  file=FILE_$currentIfNo 
  file="${!file}"

  childIfNo=${currentIfNo}0
  $TC class add dev $interface parent 1: classid 1:$currentIfNo htb rate 1024Mbps
  $TC class add dev $interface parent 1:$currentIfNo classid 1:$childIfNo htb rate 1024Mbps 
  $TC qdisc add dev $interface parent 1:$childIfNo handle 10: sfq perturb 10
  $TC filter add dev $interface parent 1:0 protocol ip prio 1 u32 match ip sport $port 0xffff flowid 1:$childIfNo
  policyLoop $interface $file $currentIfNo $childIfNo & 
  ((currentIfNo++))
done

wait

      #$TC qdisc del dev $device root 1>/dev/null 2>&1
      #$TC qdisc add dev $device root tbf rate $value burst $burst latency 1ms
