# Wine + Xvfb for running the original Ostrov.exe invisibly (reference captures).
FROM --platform=linux/amd64 ubuntu:24.04
ENV DEBIAN_FRONTEND=noninteractive
RUN dpkg --add-architecture i386 && apt-get update \
 && apt-get install -y --no-install-recommends wine wine32:i386 xvfb xauth ca-certificates \
 && rm -rf /var/lib/apt/lists/*
ENV WINEPREFIX=/wine WINEDEBUG=-all WINEARCH=win32
RUN xvfb-run -a wineboot -i && wineserver -w
COPY run.sh /run.sh
ENTRYPOINT ["/run.sh"]
