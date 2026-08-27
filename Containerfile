FROM --platform=linux/amd64 alpine:3.24 AS agent-x86_64-linux-builder
RUN apk upgrade
RUN apk add musl-dev protobuf gcc rustup
RUN rustup-init -y
ENV PATH="${PATH}:/root/.rustup/toolchains/stable-x86_64-unknown-linux-musl/bin/"
COPY . /build
WORKDIR /build
RUN cargo build --release -p agent
FROM fedora:46 AS builder
RUN dnf up -y
RUN dnf install -y cargo cmake libpq-devel openssl-devel protobuf-compiler unzip
RUN curl -fL https://deno.land/install.sh | sh
COPY . /build
WORKDIR /build
RUN cargo build --release -p apiserver -p proxy -p master -p webserver
WORKDIR /build/frontend
RUN ~/.deno/bin/deno install
RUN ~/.deno/bin/deno task build
FROM fedora:46
RUN dnf up -y
RUN dnf install -y libpq tini
COPY --from=agent-x86_64-linux-builder /build/target/release/agent agent-x86_64-linux
COPY --from=builder /build/target/release/apiserver .
COPY --from=builder /build/target/release/proxy .
COPY --from=builder /build/target/release/master .
COPY --from=builder /build/target/release/webserver .
COPY --from=builder /build/frontend/build /frontend
ENTRYPOINT ["/bin/tini"]
