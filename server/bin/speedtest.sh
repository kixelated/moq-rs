url=$1

if avg_speed=$(curl -qkfsS -w '%{speed_download}' -o /dev/null --url "$url")
then
  echo "$((avg_speed*8)) bits/sec"
fi
