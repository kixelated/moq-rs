FROM ubuntu:latest
LABEL authors="liamf"

# Install xvfb as X-Server, ffmpeg & kate
RUN apt-get update
RUN apt-get install -y xvfb
RUN apt-get install -y ffmpeg
RUN apt-get install -y kate
RUN apt-get install -y xdotool
RUN apt-get install -y alsa
RUN apt-get install -y dbus-x11
RUN apt-get install -y pulseaudio

## Install google chrome
#RUN apt-get install -y curl
#RUN curl -LO https://dl.google.com/linux/direct/google-chrome-stable_current_amd64.deb
#RUN apt-get install -y ./google-chrome-stable_current_amd64.deb
#RUN rm google-chrome-stable_current_amd64.deb

## Install python
#RUN apt-get install -y python3
#
## Install xvfbwrapper
#RUN apt-get install -y python3-xvfbwrapper
#
#ADD ./xvfbPyTest/python_test.py /usr/python_test.py
#
## Run python script
#RUN #python3 /usr/python_test.py

## Run kate in xvfb
#RUN #Xvfb :99 -screen 0 1280x720x8 -once & kate
#
## Record display :99 to out.mov
#RUN #ffmpeg -y -r 30 -f x11grab -s 1280x720 -i :99 out.mov
#
#RUN #xwd -root -silent -display :99.0 | xwdtopnm |pnmtojpeg > screen.jpg
#RUN #xvfb-run -s "-screen 0 1280x720x8" -a kate

EXPOSE 4443

COPY ./target /usr/bin/application_streamer
#RUN chmod +x /usr/bin/application_streamer/application_streamer
#
#ENTRYPOINT ["/usr/bin/application_streamer/application_streamer"]