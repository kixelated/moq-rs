#!/bin/bash
input_file="source.mp4"
segment_duration=2
chunk_duration=0.04
fps=25

ffmpeg -i $input_file \
    -f dash -ldash 1 \
    -c:v libx264 \
    -filter:v fps=$fps \
    -preset veryfast -tune zerolatency \
    -c:a aac \
    -b:a 128k -ac 2 -ar 44100 \
    -map v:0 -s:v:0 1920x1080 -b:v:0 4M  \
    -map v:0 -s:v:1 1080x720 -b:v:1 2.6M \
    -map v:0 -s:v:2 960x540  -b:v:2 1.3M \
    -map v:0 -s:v:3 640x360  -b:v:3 365k \
    -map 0:a \
    -force_key_frames "expr:gte(t,n_forced*2)" \
    -sc_threshold 0 \
    -streaming 1 \
    -use_timeline 0 \
    -seg_duration $segment_duration -frag_duration $chunk_duration \
    -frag_type duration \
    playlist.mpd
