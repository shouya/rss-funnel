APP_NAME ?= rss-funnel
IMAGE_USER ?= shouya
IMAGE_HOST ?= git.lain.li
IMAGE_NAME ?= $(IMAGE_HOST)/$(IMAGE_USER)/$(APP_NAME)

PLATFORM ?= linux/amd64
TARGET ?= x86_64-unknown-linux-musl
BINARY = target/$(TARGET)/release/$(APP_NAME)
SOURCES = $(wildcard **/*.rs) Cargo.toml Cargo.lock

VERSION ?= v$(shell git describe --tags --always --dirty)

.PHONY: inspector-assets
inspector-assets:
	cd inspector && pnpm build

target/x86_64-unknown-linux-musl/release/$(APP_NAME): $(SOURCES) inspector-assets
	cargo build --release --target x86_64-unknown-linux-musl

target/aarch64-unknown-linux-musl/release/$(APP_NAME): $(SOURCES) inspector-assets
	cross build --release --target aarch64-unknown-linux-musl

build-docker-multiarch:
	podman manifest create $(IMAGE_NAME):$(VERSION) \
		$(IMAGE_NAME):$(VERSION)-x86_64-unknown-linux-musl \
		$(IMAGE_NAME):$(VERSION)-aarch64-unknown-linux-musl
	podman tag $(IMAGE_NAME):$(VERSION) $(IMAGE_NAME):latest

build-docker-$(TARGET): $(BINARY)
	echo "FROM scratch\nCOPY $< /$(APP_NAME)\nENTRYPOINT [\"/$(APP_NAME)\"]\nCMD [\"server\"]\n" | \
		podman build -f - . \
			--platform $(PLATFORM) \
			-t $(IMAGE_NAME):latest-$(TARGET) \
			-t $(IMAGE_NAME):$(VERSION)-$(TARGET)

push-docker: build-docker-$(TARGET)
	podman push $(IMAGE_NAME):$(VERSION)-$(TARGET)
	podman push $(IMAGE_NAME):latest-$(TARGET)

push-docker-multiarch: build-docker-multiarch
	podman manifest push $(IMAGE_NAME):$(VERSION)
	podman manifest push $(IMAGE_NAME):latest

.PHONY: build-docker build-docker-multiarch push-docker push-docker-multiarch
