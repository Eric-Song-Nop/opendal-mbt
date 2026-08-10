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
MOON_LIBRARY_PATH := $(NATIVE_LIB_DIR)$(if $(LIBRARY_PATH),:$(LIBRARY_PATH))
MOON_TEST_FLAGS := --target native --frozen --warn-list '$(MOON_WARN_LIST)' --deny-warn

.PHONY: native rust-test moon-check moon-test coverage abi-smoke c-example \
	api-contract interface-contract package-contract packaged-consumer check \
	test-profile asan

native:
	cargo build --workspace --locked $(CARGO_PROFILE_FLAG)

rust-test:
	cargo test --workspace --all-targets --all-features --locked $(CARGO_PROFILE_FLAG)

moon-check:
	LIBRARY_PATH="$(MOON_LIBRARY_PATH)" \
		moon check --target native --frozen --warn-list '$(MOON_WARN_LIST)' --deny-warn

moon-test: native
	LIBRARY_PATH="$(MOON_LIBRARY_PATH)" \
		moon test $(MOON_TEST_FLAGS) $(MOON_PROFILE_FLAG)

coverage: native
	moon clean
	LIBRARY_PATH="$(MOON_LIBRARY_PATH)" \
		moon test $(MOON_TEST_FLAGS) --enable-coverage
	LIBRARY_PATH="$(MOON_LIBRARY_PATH)" moon coverage analyze

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

check: api-contract interface-contract package-contract
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
	$(MAKE) moon-check
	$(MAKE) abi-smoke

test-profile: rust-test moon-test

asan:
	$(MAKE) native RUST_PROFILE=debug
	LIBRARY_PATH="$(CURDIR)/target/debug$(if $(LIBRARY_PATH),:$(LIBRARY_PATH))" \
		python3 .agents/skills/moonbit-c-binding/scripts/run-asan.py \
			--repo-root . --pkg moon.pkg \
			--pkg integration/consumer/moon.pkg
