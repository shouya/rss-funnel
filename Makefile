APP_NAME ?= rss-funnel
IMAGE_USER ?= shouya
IMAGE_HOST ?= ghcr.io
IMAGE_NAME ?= $(IMAGE_HOST)/$(IMAGE_USER)/$(APP_NAME)

TARGETS ?= x86_64-unknown-linux-musl aarch64-unknown-linux-musl
VERSION ?= $(shell git describe --tags --always --dirty)

PLATFORM_x86_64-unknown-linux-musl = linux/amd64
PLATFORM_aarch64-unknown-linux-musl = linux/arm64/v8

SOURCES := $(wildcard **/*.rs) Cargo.toml Cargo.lock

.PHONY: \
	inspector-assets \
	$(IMAGE_NAME)\:$(VERSION) \
	$(IMAGE_NAME)\:latest \
	$(IMAGE_NAME)\:nightly \
	push-manifest-$(VERSION) \
	push-manifest-latest \
	push-manifest-nightly \
	print-version

# The following rules are skipped because "The implicit rule search (see Implicit Rules) is skipped for .PHONY targets."
# $(foreach target,$(TARGETS),$(IMAGE_NAME)\:$(VERSION)-$(target)) \
# $(foreach target,$(TARGETS),$(IMAGE_NAME)\:latest-$(target)) \
# $(foreach target,$(TARGETS),push-docker-$(VERSION)-$(target)) \

print-version:
	@echo $(IMAGE_NAME):$(VERSION)

inspector-assets:
	cd inspector && pnpm install && pnpm build

target/x86_64-unknown-linux-musl/release/$(APP_NAME): $(SOURCES) inspector-assets
	cross build --release --target x86_64-unknown-linux-musl

target/aarch64-unknown-linux-musl/release/$(APP_NAME): $(SOURCES) inspector-assets
# https://github.com/cross-rs/cross/issues/724
	cargo clean
	cross build --release --target aarch64-unknown-linux-musl

target/arm-unknown-linux-gnueabihf/release/$(APP_NAME): $(SOURCES) inspector-assets
	cargo clean
	cross build --release --target arm-unknown-linux-gnueabihf --features bindgen

$(IMAGE_NAME)\:latest-% $(IMAGE_NAME)\:nightly-%: $(IMAGE_NAME)\:$(VERSION)-%
	podman tag $< $@

$(IMAGE_NAME)\:$(VERSION)-%: target/%/release/$(APP_NAME)
	cat Dockerfile | \
		sed -e "s|%RELEASE_BINARY%|$<|g" | \
		podman build -f - . \
			--format docker \
			--platform $(PLATFORM_$*) -t $@

# building multiarch manifest requires the image to be pushed to the
# registry first.
push-image-%: $(IMAGE_NAME)\:%
	podman push $<

$(IMAGE_NAME)\:$(VERSION) $(IMAGE_NAME)\:latest $(IMAGE_NAME)\:nightly : \
$(IMAGE_NAME)\:%: $(foreach target,$(TARGETS),$(IMAGE_NAME)\:%-$(target)) \
		$(foreach target,$(TARGETS),push-image-%-$(target))
	podman manifest create $@ \
		$(foreach target,$(TARGETS),$(IMAGE_NAME)\:$*-$(target))

push-manifest-$(VERSION) push-manifest-latest push-manifest-nightly : \
push-manifest-%: $(IMAGE_NAME)\:%
	podman manifest push $< $<

build-and-push-nightly: push-manifest-$(VERSION) push-manifest-nightly
build-and-push-latest: push-manifest-$(VERSION) push-manifest-latest
