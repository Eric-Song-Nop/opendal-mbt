// MoonBit module metadata for the public facade and native binding.

name = "Eric-Song-Nop/opendal"

version = "0.2.0"

license = "Apache-2.0"

repository = "https://github.com/Eric-Song-Nop/opendal-mbt"

readme = "README.mbt.md"

preferred_target = "native"

supported_targets = "+native+js"

description = "MoonBit bindings for Apache OpenDAL"

import {
  "moonbitlang/async@0.20.5",
}

// Keep the downstream contract fixture in the development workspace without
// publishing it as part of the library module.

source = "src"

options(
  "--moonbit-unstable-prebuild": "build.js",
  exclude: [
    "integration",
    "moon.work",
    "/Makefile",
    "Cargo.lock",
    "Cargo.toml",
    "rust-toolchain.toml",
    "native/rust",
    "wasm",
    "native/distribution-profile.json",
    "native/distribution-profiles",
    "examples/c",
    "examples/browser/.mooncakes",
    "examples/browser/_build",
    "examples/browser/__pycache__",
    "examples/browser/pkg.generated.mbti",
    "tests",
    "scripts",
    "skills-lock.json",
    "/trace.json",
  ],
)
