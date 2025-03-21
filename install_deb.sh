#!/bin/bash
set -x
set -euo pipefail

arch="${1}"
shift

# need to install certain local dependencies
export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get install --assume-yes --no-install-recommends \
  ca-certificates \
  curl \
  cpio \
  sharutils \
  gnupg \
  debian-archive-keyring \
  devscripts \
  dpkg-dev

# get our debian sources
debsource="deb https://mirrors.tuna.tsinghua.edu.cn/debian/  bullseye-updates main contrib non-free"
debsource="${debsource}\ndeb https://security.debian.org/debian-security bullseye-security main contrib non-free"

# temporarily use debian sources rather than ubuntu.
mv /etc/apt/sources.list /etc/apt/sources.list.bak
echo -e "${debsource}" > /etc/apt/sources.list

dpkg --add-architecture "${arch}" || echo "foreign-architecture ${arch}" \
  > /etc/dpkg/dpkg.cfg.d/multiarch

# Add Debian keys.
curl --retry 3 -sSfL 'https://ftp-master.debian.org/keys/archive-key-{7.0,8,9,10,11,12}.asc' -O
curl --retry 3 -sSfL 'https://ftp-master.debian.org/keys/archive-key-{8,9,10,11,12}-security.asc' -O
curl --retry 3 -sSfL 'https://ftp-master.debian.org/keys/release-{7,8,9,10,11,12}.asc' -O
curl --retry 3 -sSfL 'https://www.ports.debian.org/archive_{2020,2021,2022,2023,2024}.key' -O

for key in *.asc *.key; do
  apt-key add "${key}"
  rm "${key}"
done

# allow apt-get to retry downloads
echo 'APT::Acquire::Retries "3";' > /etc/apt/apt.conf.d/80-retries

apt-get update
for dep in $@; do
  apt-get install "${dep}:${arch}" --assume-yes
done

# restore our old sources list
mv -f /etc/apt/sources.list.bak /etc/apt/sources.list
if [ -f /etc/dpkg/dpkg.cfg.d/multiarch.bak ]; then
    mv /etc/dpkg/dpkg.cfg.d/multiarch.bak /etc/dpkg/dpkg.cfg.d/multiarch
fi

# can fail if arch is used (amd64 and/or i386)
dpkg --remove-architecture "${arch}" || true
apt-get update
