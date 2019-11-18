FROM rust:1.36.0 as build

RUN echo "deb http://deb.debian.org/debian stretch-backports main" > /etc/apt/sources.list.d/backports.list \
    && apt-get update && apt-get install -y protobuf-compiler/stretch-backports cmake golang \
    && apt-get clean && rm -r /var/lib/apt/lists/*

WORKDIR /vanguard2

COPY ./Cargo.toml ./Cargo.toml
COPY ./auth ./auth
COPY ./datasrc ./datasrc
COPY ./cache ./cache
COPY ./forwarder ./forwarder
COPY ./server ./server
COPY ./src ./src

RUN cargo build --release


FROM pingcap/alpine-glibc
COPY --from=build /vanguard2/target/release/vanguard2 /vanguard2
EXPOSE 53/udp
ENTRYPOINT ["/vanguard2"]
