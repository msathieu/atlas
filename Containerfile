FROM --platform=linux/amd64 alpine:3.23 AS agent-x86_64-linux-builder
RUN apk upgrade
RUN apk add musl-dev protobuf gcc rustup
RUN rustup-init -y
ENV PATH="${PATH}:/root/.rustup/toolchains/stable-x86_64-unknown-linux-musl/bin/"
COPY . /build
WORKDIR /build
RUN cargo build --release --package agent
FROM debian:trixie AS builder
RUN apt-get update
RUN apt-get dist-upgrade -y
RUN apt-get install -y build-essential curl libpq-dev pkg-config protobuf-compiler rustup unzip
RUN curl -fL https://deno.land/install.sh | sh
RUN rustup default stable
COPY . /build
WORKDIR /build
RUN cargo build --release
WORKDIR /build/frontend
RUN ~/.deno/bin/deno install
RUN ~/.deno/bin/deno task build
FROM debian:trixie
RUN apt-get update
RUN apt-get dist-upgrade -y
RUN apt-get install -y libpq-dev tini
COPY --from=agent-x86_64-linux-builder /build/target/release/agent agent-x86_64-linux
COPY --from=builder /build/target/release/apiserver .
COPY --from=builder /build/target/release/master .
COPY --from=builder /build/target/release/webserver .
COPY --from=builder /build/frontend/build /frontend
ENTRYPOINT ["/bin/tini"]
