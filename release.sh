#!/bin/bash -e

if [[ $# -ne 2 ]]; then
    echo "Usage: $0 --stage-{1,2,3} <version>"
    exit 1
fi

if [[ -n "$(git status --porcelain)" ]]; then
    echo "Working directory is not clean"
    exit 1
fi

stage=$1
version=$2

stage1() {
    make IMAGE_HOST=ghcr.io \
	 VERSION=$version \
	 TARGET=x86_64-unknown-linux-musl \
	 PLATFORM=linux/amd64 \
	 push-docker
}

stage2() {
    # must clean before cross-compiling
    # because of https://github.com/cross-rs/cross/issues/724
    cargo clean
    make IMAGE_HOST=ghcr.io \
	 VERSION=$version \
	 TARGET=aarch64-unknown-linux-musl \
	 PLATFORM=linux/arm64/v8 \
	 push-docker

    make IMAGE_HOST=ghcr.io \
	 VERSION=$version \
	 push-docker-multiarch

}

stage3() {
    git tag -a "$version" -m "Release $version"
    git push origin "$version"

    git cliff -o CHANGELOG.md
    git commit -am "chore: update changelog"
    git push origin HEAD
}

case "$stage" in
	--stage-1)
	    stage1
	;;
	--stage-2)
	    stage2
	;;
	--stage-3)
	    stage3
	;;
	*)
	    echo "Invalid stage"
	    exit 1
	;;
esac
