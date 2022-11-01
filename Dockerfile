FROM rust

USER root

RUN mkdir /rust

COPY rust /rust

WORKDIR /rust

RUN cd starlane/starlane && cargo install --path . --features postgres --root /target starlane 
RUN cd cosmic/cosmic-cli && cargo install --path . --root /target cosmic-cli

FROM ubuntu
COPY --from=0 /target/bin/starlane /usr/bin/
COPY --from=0 /target/bin/cosmic /usr/bin/
RUN apt-get update -y
RUN apt-get install -y openssl libssl-dev
RUN cd /usr/lib && ln -s libssl.so libssl.so.1.1
CMD starlane
