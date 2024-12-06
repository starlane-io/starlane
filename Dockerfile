FROM rust
ARG FEATURES=none

USER root

RUN mkdir /rust

COPY rust /rust

WORKDIR /rust

RUN cd main/main && cargo install --path . --features $FEATURES --root /target main
RUN cd cosmic/cosmic-cli && cargo install --path . --root /target cosmic-cli

FROM ubuntu
COPY --from=0 /target/bin/starlane /usr/bin/
COPY --from=0 /target/bin/cosmic /usr/bin/
CMD starlane
