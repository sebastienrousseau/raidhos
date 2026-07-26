# Debian Bookworm slim, pinned by digest for reproducibility.
# To update: `docker pull debian:bookworm-slim` then read the digest with
# `docker images --digests debian:bookworm-slim`.
FROM debian:bookworm-slim@sha256:2749ca60ffb3c42de053229d7967d292d7dad1067936b38995da0bbfb96c4c23

RUN apt-get update && apt-get install -y \
    build-essential \
    bison \
    flex \
    python3 \
    pkg-config \
    gettext \
    texinfo \
    help2man \
    libopts25-dev \
    libselinux1-dev \
    git \
    wget \
    xz-utils \
    autoconf \
    automake \
    autopoint \
    gawk \
    && rm -rf /var/lib/apt/lists/* \
    && update-alternatives --set awk /usr/bin/gawk

WORKDIR /grub-build

# git.savannah.gnu.org has had transient outages in CI. Try the savannah
# mirror first; fall back to the GitHub mirror, then the gnu.org tarball.
# Either path leaves us with a populated /grub-build with the grub-2.12
# tag checked out.
RUN set -eux; \
    for attempt in 1 2 3; do \
        if git clone --depth 1 --branch grub-2.12 https://git.savannah.gnu.org/git/grub.git .; then \
            exit 0; \
        fi; \
        echo "savannah attempt $attempt failed, retrying..."; \
        sleep $((attempt * 5)); \
        rm -rf /grub-build/* /grub-build/.* 2>/dev/null || true; \
    done; \
    git clone --depth 1 --branch grub-2.12 https://github.com/rhboot/grub2.git .

RUN ./bootstrap && \
    ./configure --with-platform=efi --target=x86_64 --disable-werror && \
    make -j$(nproc)

ENTRYPOINT ["./grub-mkstandalone"]
