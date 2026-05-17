# Debian Bookworm slim, pinned by digest for reproducibility.
# To update: `docker pull debian:bookworm-slim` then read the digest with
# `docker images --digests debian:bookworm-slim`.
FROM debian:bookworm-slim@sha256:b6507e340c43553136f5078daa1bd49ce8b975e6e1f1d0a2b8e5b06a4d3d7c20

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
    && rm -rf /var/lib/apt/lists/*

WORKDIR /grub-build

RUN git clone --depth 1 --branch grub-2.12 https://git.savannah.gnu.org/git/grub.git .

RUN ./bootstrap && \
    ./configure --with-platform=efi --target=x86_64 --disable-werror && \
    make -j$(nproc)

ENTRYPOINT ["./grub-mkstandalone"]
