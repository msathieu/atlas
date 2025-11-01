FROM debian:trixie AS builder
RUN apt-get update
RUN apt-get dist-upgrade -y
RUN apt-get install -y build-essential curl libpq-dev pkg-config protobuf-compiler rustup unzip
RUN curl -L https://deno.land/install.sh | sh
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
COPY --from=builder /build/target/release/agent .
COPY --from=builder /build/target/release/apiserver .
COPY --from=builder /build/target/release/master .
COPY --from=builder /build/target/release/webserver .
COPY --from=builder /build/frontend/build /frontend
ENTRYPOINT ["/bin/tini"]
