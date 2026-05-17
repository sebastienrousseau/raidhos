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

RUN git clone --depth 1 --branch grub-2.12 https://git.savannah.gnu.org/git/grub.git .

RUN ./bootstrap && \
    ./configure --with-platform=efi --target=x86_64 --disable-werror && \
    make -j$(nproc)

ENTRYPOINT ["./grub-mkstandalone"]
