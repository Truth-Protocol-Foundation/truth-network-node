FROM docker.io/library/ubuntu:24.04

# show backtraces
ENV RUST_BACKTRACE 1

# install tools and dependencies
RUN apt-get update && \
	DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
	ca-certificates && \
	# apt cleanup
	apt-get autoremove -y && \
	apt-get clean && \
	find /var/lib/apt/lists/ -type f -not -name lock -delete; \
	# add system user and link ~/.local/share/polkadot to /data
	useradd --system --no-create-home --shell /usr/sbin/nologin -U polkadot && \
	mkdir -p /data /polkadot/.local/share && \
	chown -R polkadot:polkadot /data && \
	ln -s /data /polkadot/.local/share/tnf-node

USER polkadot

# copy the compiled binary to the container
COPY --chown=polkadot:polkadot --chmod=774 target/release/tnf-node /usr/bin/tnf-node

# check if executable works in this container
RUN /usr/bin/tnf-node --version

# ws_port
EXPOSE 9930 9333 9944 30333 30334

CMD ["/usr/bin/tnf-node"]
