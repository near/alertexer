FROM rust:1.68 AS build

WORKDIR /tmp/
COPY Cargo.toml Cargo.lock ./
COPY alertexer/Cargo.toml ./alertexer/
COPY alertexer-types/Cargo.toml ./alertexer-types/
COPY alert-rules/Cargo.toml ./alert-rules/
COPY shared/Cargo.toml ./shared/
COPY storage/Cargo.toml ./storage/

RUN /bin/bash -c "mkdir -p {alertexer,alertexer-types,alert-rules,shared,storage}/src" && \
    echo 'fn main() {}' > alertexer/src/main.rs && \
    touch alertexer-types/src/lib.rs && \
    touch alert-rules/src/lib.rs && \
    touch shared/src/lib.rs && \
    touch storage/src/lib.rs && \
    cargo build

COPY ./ ./

RUN cargo build --release --package alertexer


FROM ubuntu:20.04

RUN apt update && apt install -yy openssl ca-certificates

USER nobody
COPY --from=build /tmp/target/release/alertexer /alertexer
ENTRYPOINT ["/alertexer"]
