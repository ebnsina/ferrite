# Ferrite API — build from the workspace root: docker build -f deploy/api.Dockerfile .
FROM rust:1-bookworm AS build
WORKDIR /app
COPY . .
RUN cargo build --release -p ferrite-api

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /app/target/release/ferrite-api /usr/local/bin/ferrite-api
# Migrations ship with the image so they can be run on deploy.
COPY migrations /migrations
EXPOSE 8091
CMD ["ferrite-api"]
