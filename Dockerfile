# The `rsi-ralph` binary as an image. Build from a checkout with submodules
# initialized (`git submodule update --init --recursive`): the vendored hive
# language and OpenHuman are path dependencies.
#
#   docker build -t rsi-ralph .
#   docker run --rm -v "$PWD/loops:/loops" -v rsi-ralph-home:/data rsi-ralph check /loops/graph-coloring
FROM rust:1-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release --locked -p rsi-ralph-cli

FROM debian:bookworm-slim
# `git` and `diff` are shelled out to by the loop subject; python3 runs the
# step scripts of the bundled example loops.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates git diffutils python3 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/rsi-ralph /usr/local/bin/rsi-ralph
# Run records live under the home directory; mount a volume here to keep them.
ENV TINYRSI_HOME=/data
VOLUME /data
# The UI binds to 127.0.0.1 by design, so it is reachable only with
# `--network host`; otherwise pass `--no-serve` to `run`.
ENTRYPOINT ["rsi-ralph"]
