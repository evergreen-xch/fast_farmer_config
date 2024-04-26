FROM rust:1.77-bullseye  as sources
WORKDIR /app
RUN mkdir src
RUN mkdir /build
COPY .cargo .cargo
COPY Cargo.toml Cargo.toml
COPY Cargo.lock Cargo.lock
RUN echo "fn main() {println!(\"dummy\");}" > src/main.rs
RUN cargo fetch

FROM sources as build
COPY src/ src/
RUN cargo build --release
RUN mv target/release/* /build

FROM scratch as bins
COPY --from=build /build/ff_config /ff_config