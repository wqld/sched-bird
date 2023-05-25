FROM rust:latest as builder

RUN rustup target add aarch64-unknown-linux-musl
RUN apt update && apt install -y musl-tools musl-dev

WORKDIR /app

RUN cargo new sched-bird
COPY Cargo.toml Cargo.lock ./sched-bird/

WORKDIR /app/sched-bird

RUN cargo build --release --target aarch64-unknown-linux-musl
COPY src ./src
RUN touch src/main.rs
RUN cargo build --release --target aarch64-unknown-linux-musl

FROM scratch

COPY --from=builder /app/sched-bird/target/aarch64-unknown-linux-musl/release/sched-bird /sched-bird

CMD ["/sched-bird"]
