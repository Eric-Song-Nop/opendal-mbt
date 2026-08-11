// MoonBit module metadata for the public contract and future native binding.

name = "Eric-Song-Nop/opendal"

version = "0.1.0"

license = "Apache-2.0"

repository = "https://github.com/Eric-Song-Nop/opendal-mbt"

readme = "README.mbt.md"

preferred_target = "native"

description = "MoonBit bindings for Apache OpenDAL"

// Keep the downstream contract fixture in the development workspace without
// publishing it as part of the library module.

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
    "native/distribution-profile.json",
    "examples",
    "tests",
    "scripts",
    "skills-lock.json",
  ],
)
