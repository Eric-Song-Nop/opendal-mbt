import assert from "node:assert/strict";
import test from "node:test";

import {
  assertBridgeImportsResolve,
  assertSafeImports,
  checkContract,
  makeSnapshot,
  parseDefinedMemories,
} from "./check-wasm-contract.mjs";

const WASM_HEADER = [0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];

function encodeU32(value) {
  const bytes = [];
  let remaining = value >>> 0;
  do {
    let byte = remaining & 0x7f;
    remaining >>>= 7;
    if (remaining !== 0) {
      byte |= 0x80;
    }
    bytes.push(byte);
  } while (remaining !== 0);
  return bytes;
}

function wasmSection(id, payload) {
  return [id, ...encodeU32(payload.length), ...payload];
}

function wasmModule(...sections) {
  return Uint8Array.from([...WASM_HEADER, ...sections.flat()]);
}

function memoryType(flags, minimum, maximum) {
  return [
    ...encodeU32(flags),
    ...encodeU32(minimum),
    ...(maximum === undefined ? [] : encodeU32(maximum)),
  ];
}

function memoryModule(...types) {
  return wasmModule(wasmSection(5, [...encodeU32(types.length), ...types.flat()]));
}

test("defined memory parser reads 32-bit minimum and maximum limits", () => {
  assert.deepEqual(parseDefinedMemories(memoryModule(memoryType(0, 2))), [
    { minimum_pages: 2, maximum_pages: null },
  ]);
  assert.deepEqual(parseDefinedMemories(memoryModule(memoryType(1, 2, 9))), [
    { minimum_pages: 2, maximum_pages: 9 },
  ]);
});

test("defined memory parser rejects shared memory", () => {
  assert.throws(
    () => parseDefinedMemories(memoryModule(memoryType(3, 1, 2))),
    /shared memory is not supported/,
  );
});

test("defined memory parser rejects memory64", () => {
  assert.throws(
    () => parseDefinedMemories(memoryModule(memoryType(4, 1))),
    /memory64 is not supported/,
  );
});

test("defined memory parser rejects unknown limits flags", () => {
  assert.throws(
    () => parseDefinedMemories(memoryModule(memoryType(8, 1))),
    /unknown limits flags 0x8/,
  );
});

test("defined memory parser rejects overflowing flag encodings", () => {
  const payload = [1, 0x80, 0x80, 0x80, 0x80, 0x10, 1];
  assert.throws(
    () => parseDefinedMemories(wasmModule(wasmSection(5, payload))),
    /memory 0 flags exceeds u32/,
  );
});

test("defined memory parser rejects truncated and trailing memory payloads", () => {
  assert.throws(
    () => parseDefinedMemories(wasmModule(wasmSection(5, [1, 1, 2]))),
    /truncated Wasm while reading memory 0 maximum/,
  );
  assert.throws(
    () => parseDefinedMemories(wasmModule(wasmSection(5, [1, 0, 1, 0]))),
    /memory section contains trailing bytes/,
  );
});

test("defined memory parser rejects limits and section framing attacks", () => {
  assert.throws(
    () => parseDefinedMemories(memoryModule(memoryType(1, 3, 2))),
    /maximum 2 is below minimum 3/,
  );
  assert.throws(
    () => parseDefinedMemories(wasmModule([5, 4, 1, 0, 1])),
    /section 5 extends beyond the Wasm binary/,
  );
  assert.throws(
    () =>
      parseDefinedMemories(
        wasmModule(wasmSection(5, [0]), wasmSection(5, [0])),
      ),
    /duplicate Wasm memory section/,
  );
  assert.throws(
    () => parseDefinedMemories(wasmModule(wasmSection(14, []))),
    /unknown Wasm section id 14/,
  );
});

test("defined memory parser rejects invalid headers", () => {
  assert.throws(
    () => parseDefinedMemories(Uint8Array.from([0, 97, 115, 109, 2, 0, 0, 0])),
    /invalid Wasm magic or version/,
  );
});

test("browser contract rejects every memory import and env or WASI function", () => {
  assert.throws(
    () =>
      assertSafeImports("fixture", [
        { module: "other", name: "memory", kind: "memory" },
      ]),
    /imports memory/,
  );
  for (const module of [
    "env",
    "ENV",
    "wasip1",
    "wasi_snapshot_preview1",
    "wasi:filesystem/types",
  ]) {
    assert.throws(
      () =>
        assertSafeImports("fixture", [
          { module, name: "malicious", kind: "function" },
        ]),
      /forbidden/,
    );
  }
});

test("MoonBit bridge imports must be a kind-preserving subset of Rust exports", () => {
  const imports = [
    { module: "opendal_mbt_bridge", name: "read", kind: "function" },
    { module: "opendal_mbt_host", name: "wait", kind: "function" },
  ];
  assert.doesNotThrow(() =>
    assertBridgeImportsResolve("opendal_mbt_bridge", imports, [
      { name: "read", kind: "function" },
    ]),
  );
  assert.throws(
    () =>
      assertBridgeImportsResolve("opendal_mbt_bridge", imports, [
        { name: "read", kind: "global" },
      ]),
    /is not exported by the Rust bridge/,
  );
});

test("observed sizes are baselines while caps remain failure thresholds", () => {
  const rust = {
    imports: [],
    exports: [
      { name: "bridge_call", kind: "function" },
      { name: "memory", kind: "memory" },
    ],
    defined_memories: [{ minimum_pages: 1, maximum_pages: null }],
    sizes: { raw: 100, gzip: 80, brotli: 70 },
  };
  const glue = { raw: 20, gzip: 10, brotli: 9 };
  const moon = {
    imports: [
      {
        module: "opendal_mbt_bridge",
        name: "bridge_call",
        kind: "function",
      },
    ],
    exports: [{ name: "memory", kind: "memory" }],
    defined_memories: [{ minimum_pages: 1, maximum_pages: null }],
    sizes: { raw: 50, gzip: 30, brotli: 25 },
  };
  const snapshot = makeSnapshot(rust, glue, moon);
  const changedRust = {
    ...rust,
    sizes: { raw: 101, gzip: 79, brotli: 72 },
  };
  assert.doesNotThrow(() => checkContract(snapshot, changedRust, glue, moon));

  const cappedSnapshot = structuredClone(snapshot);
  cappedSnapshot.artifacts.rust_bridge.size_caps.raw = 100;
  assert.throws(
    () => checkContract(cappedSnapshot, changedRust, glue, moon),
    /raw size 101 exceeds cap 100/,
  );

  const glueCappedSnapshot = structuredClone(snapshot);
  glueCappedSnapshot.artifacts.rust_bridge_glue.size_caps.raw = 19;
  assert.throws(
    () => checkContract(glueCappedSnapshot, rust, glue, moon),
    /Rust bridge glue raw size 20 exceeds cap 19/,
  );
});

test("exact import and export snapshots reject surface drift", () => {
  const rust = {
    imports: [],
    exports: [
      { name: "bridge_call", kind: "function" },
      { name: "memory", kind: "memory" },
    ],
    defined_memories: [{ minimum_pages: 1, maximum_pages: null }],
    sizes: { raw: 100, gzip: 80, brotli: 70 },
  };
  const glue = { raw: 20, gzip: 10, brotli: 9 };
  const moon = {
    imports: [
      {
        module: "opendal_mbt_bridge",
        name: "bridge_call",
        kind: "function",
      },
    ],
    exports: [{ name: "memory", kind: "memory" }],
    defined_memories: [{ minimum_pages: 1, maximum_pages: null }],
    sizes: { raw: 50, gzip: 30, brotli: 25 },
  };
  const snapshot = makeSnapshot(rust, glue, moon);
  const changedImports = {
    ...moon,
    imports: [
      ...moon.imports,
      { module: "opendal_mbt_host", name: "extra", kind: "function" },
    ],
  };
  assert.throws(
    () => checkContract(snapshot, rust, glue, changedImports),
    /MoonBit canary imports changed/,
  );

  const changedExports = {
    ...rust,
    exports: [
      ...rust.exports,
      { name: "new_call", kind: "function" },
    ],
  };
  assert.throws(
    () => checkContract(snapshot, changedExports, glue, moon),
    /Rust bridge exports changed/,
  );
});

test("wasm-bindgen closure hashes may vary while kind and count remain exact", () => {
  const rust = {
    imports: [],
    exports: [
      { name: "bridge_call", kind: "function" },
      {
        name: "wasm_bindgen__convert__closures_____invoke__h0d75f8712a5f6417",
        kind: "function",
      },
      {
        name: "wasm_bindgen__convert__closures_____invoke__hacf999f7d4cb3596",
        kind: "function",
      },
      { name: "memory", kind: "memory" },
    ],
    defined_memories: [{ minimum_pages: 1, maximum_pages: null }],
    sizes: { raw: 100, gzip: 80, brotli: 70 },
  };
  const glue = { raw: 20, gzip: 10, brotli: 9 };
  const moon = {
    imports: [
      {
        module: "opendal_mbt_bridge",
        name: "bridge_call",
        kind: "function",
      },
    ],
    exports: [{ name: "memory", kind: "memory" }],
    defined_memories: [{ minimum_pages: 1, maximum_pages: null }],
    sizes: { raw: 50, gzip: 30, brotli: 25 },
  };
  const snapshot = makeSnapshot(rust, glue, moon);
  assert.deepEqual(snapshot.artifacts.rust_bridge.generated_exports, [
    {
      name_prefix: "wasm_bindgen__convert__closures_____invoke__h",
      hash_encoding: "lower_hex_16",
      kind: "function",
      count: 2,
    },
  ]);

  const platformVariant = {
    ...rust,
    exports: rust.exports.map((entry) => {
      if (entry.name.endsWith("0d75f8712a5f6417")) {
        return {
          ...entry,
          name: "wasm_bindgen__convert__closures_____invoke__h5ed240abe4fef72e",
        };
      }
      if (entry.name.endsWith("acf999f7d4cb3596")) {
        return {
          ...entry,
          name: "wasm_bindgen__convert__closures_____invoke__ha938a1bf75da0f4f",
        };
      }
      return entry;
    }),
  };
  assert.doesNotThrow(() => checkContract(snapshot, platformVariant, glue, moon));

  const unexpectedProjectExport = {
    ...platformVariant,
    exports: [
      ...platformVariant.exports,
      { name: "opendal_mbt_wasm_unreviewed_call", kind: "function" },
    ],
  };
  assert.throws(
    () => checkContract(snapshot, unexpectedProjectExport, glue, moon),
    /Rust bridge exports changed/,
  );

  const extraGeneratedExport = {
    ...platformVariant,
    exports: [
      ...platformVariant.exports,
      {
        name: "wasm_bindgen__convert__closures_____invoke__h0123456789abcdef",
        kind: "function",
      },
    ],
  };
  assert.throws(
    () => checkContract(snapshot, extraGeneratedExport, glue, moon),
    /Rust bridge generated exports changed/,
  );
});
