
FROM rust:1.95.0-slim AS build

WORKDIR /app

RUN apt-get update && apt-get install -y protobuf-compiler

COPY Cargo.toml Cargo.lock build.rs registry.proto task.proto ./
COPY src/ src/

RUN cargo build --release

FROM gcr.io/distroless/cc-debian12:latest

COPY --from=build /app/target/release/enclave_runner /app/enclave_runner

CMD [ "/app/enclave_runner" ]