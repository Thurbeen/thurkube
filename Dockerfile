# syntax=docker/dockerfile:1

# --- Builder stage ---
FROM rust:1-bookworm AS builder

WORKDIR /build

# Cache dependencies by building a dummy project first
COPY Cargo.toml Cargo.lock build.rs ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs && echo '' > src/lib.rs \
    && cargo build --release \
    && rm -rf src

# Build the real binary
COPY src/ src/
ARG THURKUBE_RELEASE_VERSION
ENV THURKUBE_RELEASE_VERSION=${THURKUBE_RELEASE_VERSION}
RUN touch src/main.rs src/lib.rs \
    && cargo build --release

# --- Runtime stage ---
FROM gcr.io/distroless/cc-debian12:nonroot

COPY --from=builder /build/target/release/thurkube /thurkube

USER nonroot:nonroot

ENTRYPOINT ["/thurkube"]
