FROM rust:latest as builder

RUN rustup target add aarch64-unknown-linux-musl
RUN apt-get update && apt-get install -y musl-tools musl-dev clang llvm pkg-config libssl-dev
ENV CC_aarch64_unknown_linux_musl=clang
ENV AR_aarch64_unknown_linux_musl=llvm-ar
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_RUSTFLAGS="-Clink-self-contained=yes -Clinker=rust-lld"

WORKDIR /app

RUN cargo new sched-bird
COPY Cargo.toml Cargo.lock ./sched-bird/

WORKDIR /app/sched-bird

RUN cargo build --release --target aarch64-unknown-linux-musl
COPY src ./src
RUN touch src/main.rs
RUN cargo build --release --target aarch64-unknown-linux-musl

# CMD ["/app/sched-bird/target/aarch64-unknown-linux-musl/release/sched-bird"]

FROM scratch

COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=builder /app/sched-bird/target/aarch64-unknown-linux-musl/release/sched-bird /sched-bird

CMD ["/sched-bird"]
