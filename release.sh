#!/bin/bash -e

if [[ $# -ne 1 ]]; then
    echo "Usage: $0 <version>"
    echo "Example: $0 0.1.3"
    exit 1
fi

if [[ -n "$(git status --porcelain)" ]]; then
    echo "Working directory is not clean"
    exit 1
fi

version=$1
git cliff -o CHANGELOG.md
git commit -am "chore: update changelog"
git tag -a "$version" -m "Release $version"

git push origin HEAD && git push origin "$version"

make build-and-push-latest
