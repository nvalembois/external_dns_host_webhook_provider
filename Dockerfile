FROM docker.io/library/rust:1.81.0-alpine3.20 AS build

COPY src/ Cargo.lock Cargo.toml /tmp/

WORKDIR /tmp

RUN set -e && \
  apk add --no-cache musl-dev build-base && \
  cat Cargo.toml && \
  cargo build --release

FROM docker.io/library/alpine:3.20.3

COPY --from=build /tmp/target/release/host_webhook_provider /

USER 10000
CMD [ "/host_webhook_provider" ]