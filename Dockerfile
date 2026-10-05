# syntax=docker/dockerfile:1

# musicbanana in one image: the Rust server and the static frontend it serves.
# Node and the Rust toolchain are only needed in the build stages.
#
#   docker build -t musicbanana .
#
# deploy/compose.yaml runs it together with PostgreSQL.

FROM node:22-slim AS frontend
WORKDIR /src/frontend
COPY frontend/package.json frontend/pnpm-lock.yaml frontend/pnpm-workspace.yaml ./
# The pnpm version pinned in package.json (packageManager), as in CI.
RUN --mount=type=cache,target=/root/.local/share/pnpm/store \
    npm install --global "$(node -p 'require("./package.json").packageManager')" \
 && pnpm install --frozen-lockfile
COPY frontend/ ./
RUN pnpm run build

FROM rust:1-slim-trixie AS backend
WORKDIR /src/backend
# Queries are checked against the committed cache in backend/.sqlx.
ENV SQLX_OFFLINE=true
# Shown on the status page; CI passes the commit.
ARG GIT_COMMIT=
COPY backend/ ./
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/backend/target \
    cargo build --release --locked \
 && cp target/release/musicbanana /usr/local/bin/musicbanana

FROM debian:trixie-slim
# Root certificates for HTTPS requests, such as to YourSpotify.
RUN apt-get update \
 && apt-get install --yes --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/*
RUN useradd --system --no-create-home --uid 10001 musicbanana
COPY --from=backend /usr/local/bin/musicbanana /usr/local/bin/musicbanana
COPY --from=frontend /src/frontend/build /usr/share/musicbanana
ENV LISTEN_ADDR=0.0.0.0:3000 \
    STATIC_DIR=/usr/share/musicbanana \
    RUST_LOG=musicbanana=info,tower_http=info
USER musicbanana
EXPOSE 3000
# Subcommands work as arguments, e.g. `docker compose run --rm musicbanana token list`.
ENTRYPOINT ["musicbanana"]
CMD ["serve"]
