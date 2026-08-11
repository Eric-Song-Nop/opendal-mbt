SHELL := /bin/sh

RUST_PROFILE ?= debug
MOON_WARN_LIST ?= -68+73

ifeq ($(RUST_PROFILE),debug)
CARGO_PROFILE_FLAG :=
MOON_PROFILE_FLAG :=
else ifeq ($(RUST_PROFILE),release)
CARGO_PROFILE_FLAG := --release
MOON_PROFILE_FLAG := --release
else
$(error RUST_PROFILE must be debug or release)
endif

NATIVE_LIB_DIR := $(CURDIR)/target/$(RUST_PROFILE)
MOON_NATIVE_LIB := $(NATIVE_LIB_DIR)/libopendal_mbt_native.a
MOON_TEST_FLAGS := --target native --frozen --warn-list '$(MOON_WARN_LIST)' --deny-warn


.PHONY: native rust-test moon-check moon-test coverage abi-smoke c-example \
	api-contract interface-contract package-contract packaged-consumer check \
	test-profile native-artifact-test asan

native:
	cargo build --workspace --locked $(CARGO_PROFILE_FLAG)

rust-test:
	cargo test --workspace --all-targets --all-features --locked $(CARGO_PROFILE_FLAG)

moon-check:
	moon check --target native --frozen --warn-list '$(MOON_WARN_LIST)' --deny-warn

moon-test: native
	OPENDAL_MBT_NATIVE_LIB="$(MOON_NATIVE_LIB)" \
		moon test $(MOON_TEST_FLAGS) $(MOON_PROFILE_FLAG)

coverage: native
	moon clean
	OPENDAL_MBT_NATIVE_LIB="$(MOON_NATIVE_LIB)" \
		moon test $(MOON_TEST_FLAGS) --enable-coverage
	moon coverage analyze

abi-smoke:
	$${CC:-cc} -std=c11 -Wall -Wextra -Werror -Wpedantic \
		-fsyntax-only tests/c/abi_header_smoke.c
	$${CXX:-c++} -std=c++17 -Wall -Wextra -Werror -Wpedantic \
		-x c++ -fsyntax-only tests/c/abi_header_smoke.c
	$(MAKE) -C examples/c syntax

c-example:
	./scripts/run-c-example.sh $(RUST_PROFILE)

api-contract:
	sh scripts/check-public-api.sh

interface-contract:
	moon info --target native --frozen operator.mbt
	git diff --exit-code -- pkg.generated.mbti

package-contract:
	sh scripts/check-package.sh

packaged-consumer:
	sh scripts/check-packaged-consumer.sh $(RUST_PROFILE)

native-artifact-test:
	python3 scripts/test-package-native-artifact.py
	node --test scripts/test-native-resolver.js

check: api-contract interface-contract package-contract native-artifact-test
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
	$(MAKE) moon-check
	$(MAKE) abi-smoke

test-profile: rust-test moon-test

asan:
	$(MAKE) native RUST_PROFILE=debug
	OPENDAL_MBT_NATIVE_LIB="$(CURDIR)/target/debug/libopendal_mbt_native.a" \
		python3 .agents/skills/moonbit-c-binding/scripts/run-asan.py \
			--repo-root . --pkg moon.pkg \
			--pkg integration/consumer/moon.pkg
