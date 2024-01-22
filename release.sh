#!/bin/bash -e

if [[ $# -ne 1 ]]; then
    echo "Usage: $0 <version>"
    exit 1
fi

if [[ -n "$(git status --porcelain)" ]]; then
    echo "Working directory is not clean"
    exit 1
fi

version=$1

make IMAGE_HOST=ghcr.io \
     VERSION=$version \
     TARGET=x86_64-unknown-linux-musl \
     PLATFORM=linux/amd64 \
     push-docker

make IMAGE_HOST=ghcr.io \
     VERSION=$version \
     TARGET=aarch64-unknown-linux-musl \
     PLATFORM=linux/arm64/v8 \
     push-docker

make IMAGE_HOST=ghcr.io \
     VERSION=$version \
     push-docker-multiarch

git tag -a "$version" -m "Release $version"
git push origin "$version"

git cliff -o CHANGELOG.md
git commit -am "chore: update changelog"
git push origin HEAD
