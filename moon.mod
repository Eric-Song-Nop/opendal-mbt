// MoonBit module metadata for the public contract and future native binding.

name = "eric-song-nop/opendal"

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
    "scripts/package-native-artifact.py",
    "scripts/test-package-native-artifact.py",
    "scripts/test-native-resolver.js",
    "scripts/check-*",
    "skills-lock.json",
  ],
)
