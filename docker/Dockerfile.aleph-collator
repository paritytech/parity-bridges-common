FROM ubuntu:jammy-20220531

COPY target/release/aleph-parachain-node /usr/local/bin
RUN chmod +x /usr/local/bin/aleph-parachain-node

EXPOSE 30333 9933 9944

ENTRYPOINT ["/usr/local/bin/aleph-parachain-node"]

