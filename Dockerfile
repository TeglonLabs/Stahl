FROM rust:slim AS build

COPY . /stahl/

WORKDIR /stahl

RUN apt update && \
		apt install -y \
		build-essential \
		libssl-dev \
		openssl \
		pkg-config

RUN mkdir -p /lib/stahl/

ENV STAHL_HOME="/lib/stahl"

RUN cargo build --release

RUN cargo install --path crates/cargo-stahl-lib

RUN cd cogs && cargo run -- install.scm

FROM rust:slim

COPY --from=build /stahl/target/release/stahl /usr/local/bin

COPY --from=build /lib/stahl /lib/

ENV STAHL_HOME="/lib/stahl"

CMD ["stahl"]
