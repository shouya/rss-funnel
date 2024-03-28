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
	push-docker-latest

# The following rules are skipped because "The implicit rule search (see Implicit Rules) is skipped for .PHONY targets."
# $(foreach target,$(TARGETS),$(IMAGE_NAME)\:$(VERSION)-$(target)) \
# $(foreach target,$(TARGETS),$(IMAGE_NAME)\:latest-$(target)) \
# $(foreach target,$(TARGETS),push-docker-$(VERSION)-$(target)) \

inspector-assets:
	cd inspector && pnpm install && pnpm build

target/x86_64-unknown-linux-musl/release/$(APP_NAME): $(SOURCES) inspector-assets
	cargo build --release --target x86_64-unknown-linux-musl

target/aarch64-unknown-linux-musl/release/$(APP_NAME): $(SOURCES) inspector-assets
# https://github.com/cross-rs/cross/issues/724
	cargo clean
	cross build --release --target aarch64-unknown-linux-musl

$(IMAGE_NAME)\:latest-% $(IMAGE_NAME)\:nightly-%: $(IMAGE_NAME)\:$(VERSION)-%
	podman tag $< $@

$(IMAGE_NAME)\:$(VERSION)-%: target/%/release/$(APP_NAME)
	echo "FROM scratch\nCOPY $< /$(APP_NAME)\nENTRYPOINT [\"/$(APP_NAME)\"]\nCMD [\"server\"]\n" | \
		podman build -f - . --platform $(PLATFORM_$*) -t $@

# building multiarch manifest requires the image to be pushed to the
# registry first.
push-docker-$(VERSION)-%: $(IMAGE_NAME)\:$(VERSION)-%
	podman push $<

$(IMAGE_NAME)\:$(VERSION) $(IMAGE_NAME)\:latest $(IMAGE_NAME)\:nightly : \
$(IMAGE_NAME)\:%: $(foreach target,$(TARGETS),$(IMAGE_NAME)\:%-$(target)) \
		$(foreach target,$(TARGETS),push-docker-%-$(target))
	podman manifest create $@ \
		$(foreach target,$(TARGETS),$(IMAGE_NAME)\:$*-$(target))

push-docker-$(VERSION) push-docker-latest push-docker-nightly : \
push-docker-%: $(IMAGE_NAME)\:%
	podman manifest push $< $<

build-and-push-nightly: push-docker-$(VERSION) push-docker-nightly
build-and-push-latest: push-docker-$(VERSION) push-docker-latest
