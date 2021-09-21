FROM rust:1.55 as builder

RUN rustup update nightly && rustup default nightly

RUN mkdir -p /usr/src/starlane

WORKDIR /usr/src/starlane

COPY rust .

WORKDIR /usr/src/starlane/starlane

RUN cargo fetch 

RUN cargo install --path .



FROM alpine:3.13.5

COPY --from=builder /usr/local/cargo/bin/starlane /usr/local/bin/starlane

WORKDIR /tmp

RUN wget -q -O /etc/apk/keys/sgerrand.rsa.pub https://alpine-pkgs.sgerrand.com/sgerrand.rsa.pub && \
    wget https://github.com/sgerrand/alpine-pkg-glibc/releases/download/2.33-r0/glibc-2.33-r0.apk && \
    wget https://github.com/sgerrand/alpine-pkg-glibc/releases/download/2.33-r0/glibc-bin-2.33-r0.apk && \
    apk add glibc-2.33-r0.apk && \
    apk add glibc-bin-2.33-r0.apk && \
    rm -rf /tmp/*

WORKDIR /

RUN apk add --no-cache sqlite-libs gcc

ENTRYPOINT ["starlane"]
