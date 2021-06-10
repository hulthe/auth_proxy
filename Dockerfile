###################
### BUILD STAGE ###
###################
FROM rust:1.52 as build_stage

# Install build dependencies
RUN rustup target add x86_64-unknown-linux-musl

# Build project
WORKDIR /app
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl
RUN strip target/x86_64-unknown-linux-musl/release/auth_proxy

########################
### PRODUCTION STAGE ###
########################
FROM scratch

EXPOSE 3000

WORKDIR /

# Copy application binary
COPY --from=build_stage /app/target/x86_64-unknown-linux-musl/release/auth_proxy auth_proxy

CMD ["/auth_proxy", "--help"]
