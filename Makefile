SHELL := /bin/sh

RUST_PROFILE ?= debug
NATIVE_SERVICE_PROFILE ?= standard
MOON_WARN_LIST ?= -68+73
NATIVE_ARTIFACT ?=
NATIVE_ARTIFACT_TABLE ?=
WASM_BINDGEN_VERSION ?= 0.2.127

ifeq ($(RUST_PROFILE),debug)
CARGO_PROFILE_FLAG :=
MOON_PROFILE_FLAG :=
else ifeq ($(RUST_PROFILE),release)
CARGO_PROFILE_FLAG := --release
MOON_PROFILE_FLAG := --release
else
$(error RUST_PROFILE must be debug or release)
endif

ifeq ($(NATIVE_SERVICE_PROFILE),local)
CARGO_SERVICE_FLAGS := --no-default-features --features profile-local
else ifeq ($(NATIVE_SERVICE_PROFILE),standard)
CARGO_SERVICE_FLAGS := --no-default-features --features profile-standard
else
$(error NATIVE_SERVICE_PROFILE must be local or standard)
endif

NATIVE_LIB_DIR := $(CURDIR)/target/$(RUST_PROFILE)
MOON_NATIVE_LIB := $(NATIVE_LIB_DIR)/libopendal_mbt_native.a
MOON_TEST_FLAGS := --target native --frozen --warn-list '$(MOON_WARN_LIST)' --deny-warn
BROWSER_RUST_TARGET := wasm32-unknown-unknown
BROWSER_BRIDGE_STEM := opendal_mbt_browser_bridge
BROWSER_BRIDGE_RAW := $(CURDIR)/target/$(BROWSER_RUST_TARGET)/$(RUST_PROFILE)/$(BROWSER_BRIDGE_STEM).wasm
BROWSER_BRIDGE_DIR := $(CURDIR)/target/browser-js/$(RUST_PROFILE)


.PHONY: native rust-test moon-deps moon-check moon-test coverage abi-smoke c-example \
	api-contract interface-contract package-contract packaged-consumer check \
	test-profile native-artifact-test version-contract asan browser-bridge \
	browser-rust-check browser-rust-test browser-js-canary

native:
	cargo build --package opendal-mbt-native --locked $(CARGO_SERVICE_FLAGS) \
		$(CARGO_PROFILE_FLAG)

rust-test:
	cargo test --package opendal-mbt-native --all-targets --locked $(CARGO_SERVICE_FLAGS) \
		$(CARGO_PROFILE_FLAG)

browser-bridge:
	@command -v wasm-bindgen >/dev/null 2>&1 || { \
		echo "wasm-bindgen $(WASM_BINDGEN_VERSION) is required" >&2; exit 1; \
	}
	@test "$$(wasm-bindgen --version)" = "wasm-bindgen $(WASM_BINDGEN_VERSION)" || { \
		echo "expected wasm-bindgen $(WASM_BINDGEN_VERSION), got $$(wasm-bindgen --version)" >&2; \
		exit 1; \
	}
	mkdir -p "$(BROWSER_BRIDGE_DIR)"
	CARGO_PROFILE_RELEASE_PANIC=abort cargo build --locked \
		--package opendal-mbt-browser-bridge \
		--target "$(BROWSER_RUST_TARGET)" $(CARGO_PROFILE_FLAG)
	wasm-bindgen --target web --no-typescript \
		--out-dir "$(BROWSER_BRIDGE_DIR)" \
		--out-name "$(BROWSER_BRIDGE_STEM)" "$(BROWSER_BRIDGE_RAW)"
	mv -f "$(BROWSER_BRIDGE_DIR)/$(BROWSER_BRIDGE_STEM).js" \
		"$(BROWSER_BRIDGE_DIR)/$(BROWSER_BRIDGE_STEM).mjs"

browser-rust-check:
	cargo fmt --all -- --check
	cargo clippy --locked --package opendal-mbt-browser-bridge --all-targets \
		--target "$(BROWSER_RUST_TARGET)" -- -D warnings

browser-rust-test:
	cargo test --locked --package opendal-mbt-browser-bridge --lib

browser-js-canary: browser-bridge
	node wasm/browser-canary/run.mjs "$(BROWSER_BRIDGE_DIR)"

moon-deps:
	moon update
	# Dependency resolution is target-independent. Using wasm here avoids
	# requiring an as-yet-unpublished native artifact during release PRs.
	moon check --target wasm

moon-check:
	moon check --target native --frozen --warn-list '$(MOON_WARN_LIST)' --deny-warn

moon-test: native
	OPENDAL_MBT_NATIVE_LIB="$(MOON_NATIVE_LIB)" \
		OPENDAL_MBT_SOURCE_PROFILE="$(NATIVE_SERVICE_PROFILE)" \
		moon test $(MOON_TEST_FLAGS) $(MOON_PROFILE_FLAG)

coverage: native
	moon clean
	OPENDAL_MBT_NATIVE_LIB="$(MOON_NATIVE_LIB)" \
		OPENDAL_MBT_SOURCE_PROFILE="$(NATIVE_SERVICE_PROFILE)" \
		moon test $(MOON_TEST_FLAGS) --enable-coverage
	OPENDAL_MBT_NATIVE_LIB="$(MOON_NATIVE_LIB)" \
		OPENDAL_MBT_SOURCE_PROFILE="$(NATIVE_SERVICE_PROFILE)" \
		moon coverage analyze

abi-smoke:
	$${CC:-cc} -std=c11 -Wall -Wextra -Werror -Wpedantic \
		-fsyntax-only tests/c/abi_header_smoke.c
	$${CXX:-c++} -std=c++17 -Wall -Wextra -Werror -Wpedantic \
		-x c++ -fsyntax-only tests/c/abi_header_smoke.c
	$${CC:-cc} -std=c11 -Wall -Wextra -Werror -Wpedantic \
		-fsyntax-only tests/c/library_info_probe.c
	$${CC:-cc} -std=c11 -Wall -Wextra -Werror -Wpedantic \
		-fsyntax-only tests/c/async_completion_probe.c
	$(MAKE) -C examples/c syntax

c-example:
	./scripts/run-c-example.sh $(RUST_PROFILE)

api-contract:
	sh scripts/check-public-api.sh

interface-contract:
	moon info --target native --frozen src/operator.mbt
	git diff --exit-code -- src/pkg.generated.mbti

package-contract:
	sh scripts/check-package.sh

packaged-consumer: moon-deps
	test -n "$(NATIVE_ARTIFACT)"
	@if [ -n "$(NATIVE_ARTIFACT_TABLE)" ]; then \
		sh scripts/check-packaged-consumer.sh --artifact-table \
			"$(NATIVE_ARTIFACT_TABLE)" "$(NATIVE_ARTIFACT)"; \
	else \
		sh scripts/check-packaged-consumer.sh "$(NATIVE_ARTIFACT)"; \
	fi

version-contract:
	python3 scripts/test-version-metadata.py
	python3 scripts/test-render-registry-consumer.py
	python3 scripts/check-version-metadata.py

native-artifact-test:
	python3 scripts/test-distribution-profiles.py
	python3 scripts/test-package-native-artifact.py
	python3 scripts/test-native-library-info.py
	node --check scripts/prepare-test-native-cache.js
	node --test scripts/test-native-resolver.js

check: api-contract interface-contract package-contract native-artifact-test \
	version-contract
	cargo fmt --all -- --check
	cargo clippy --package opendal-mbt-native --all-targets --all-features \
		--locked -- -D warnings
	$(MAKE) moon-check
	$(MAKE) abi-smoke

test-profile: rust-test moon-test

asan:
	$(MAKE) native RUST_PROFILE=debug
	OPENDAL_MBT_NATIVE_LIB="$(CURDIR)/target/debug/libopendal_mbt_native.a" \
		OPENDAL_MBT_SOURCE_PROFILE="$(NATIVE_SERVICE_PROFILE)" \
		python3 .agents/skills/moonbit-c-binding/scripts/run-asan.py \
			--repo-root . --pkg src/moon.pkg \
			--pkg integration/consumer/moon.pkg \
			--pkg integration/s3/moon.pkg
