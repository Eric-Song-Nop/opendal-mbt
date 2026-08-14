# Documentation

OpenDAL for MoonBit supports the native and JavaScript targets through one
root package. Start with the guide that matches the job you are doing.

## Using the MoonBit package

- [Overview and installation](../src/README.mbt.md)
- [Getting started](../src/getting-started.mbt.md)
- [Connecting to storage services](../src/connecting.mbt.md)
- [Common storage tasks](../src/tasks.mbt.md)
- [Browser and JavaScript guide](../src/browser-guide.mbt.md)

The five `*.mbt.md` guides contain executable MoonBit examples. Native examples
are checked with the native target, while portable examples are checked with
both native and JavaScript targets.

## Design and maintenance

- [Public API semantics](design/public-api-semantics.md)
- [Native C ABI](design/c-abi.md)
- [Native artifact distribution](design/native-distribution.md)
- [Browser runtime architecture](design/browser-runtime.md)
- [Browser and JavaScript API reference](reference/browser-api.md)
- [Release process](releasing.md)
- [Roadmap](roadmap.md)

Component-specific documentation lives beside the component:

- [Browser bridge and embedded runtime](https://github.com/Eric-Song-Nop/opendal-mbt/blob/v0.2.0/wasm/README.md)
- [Native/browser async and non-blocking I/O example](../examples/browser/README.md)
- [C examples](https://github.com/Eric-Song-Nop/opendal-mbt/blob/v0.2.0/examples/c/README.md)

## API reference

The checked-in native interface is
[`src/pkg.generated.mbti`](../src/pkg.generated.mbti). Query the target-specific
API, including JavaScript-only extensions, with Moon's semantic documentation
command:

```sh
moon ide doc --target native '@Eric-Song-Nop/opendal'
moon ide doc --target js '@Eric-Song-Nop/opendal'
```

Public declarations carry API docstrings on both targets. The generated C ABI
reference is [`native/include/opendal_mbt.h`](../native/include/opendal_mbt.h),
and the machine-readable Browser ABI contract is
[`wasm/browser-runtime/contract.json`](https://github.com/Eric-Song-Nop/opendal-mbt/blob/v0.2.0/wasm/browser-runtime/contract.json).

## Checking documentation

From the repository root:

```sh
make docs-check
make moon-test
make moon-browser-test
```

`docs-check` verifies local links, anchors, balanced code fences, and explicit
`mbt check`/`mbt nocheck` annotations. The Moon commands compile and run the
examples embedded in the user guides.
