#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import { isDeepStrictEqual } from "node:util";
import { pathToFileURL } from "node:url";
import {
  brotliCompressSync,
  constants as zlibConstants,
  gzipSync,
} from "node:zlib";

const WASM_HEADER = Uint8Array.of(0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00);
const MEMORY_SECTION = 5;
const LAST_KNOWN_SECTION = 13;
const BRIDGE_IMPORT_MODULE = "opendal_mbt_bridge";

const DEFAULT_SIZE_CAPS = Object.freeze({
  rust_bridge: Object.freeze({ raw: 1_100_000, gzip: 360_000, brotli: 280_000 }),
  rust_bridge_glue: Object.freeze({ raw: 16_384, gzip: 4_096, brotli: 4_096 }),
  moonbit_canary: Object.freeze({ raw: 65_536, gzip: 24_576, brotli: 20_480 }),
});

class BinaryReader {
  constructor(bytes, start = 0, end = bytes.length) {
    this.bytes = bytes;
    this.offset = start;
    this.end = end;
  }

  byte(context) {
    if (this.offset >= this.end) {
      throw new Error(`truncated Wasm while reading ${context}`);
    }
    return this.bytes[this.offset++];
  }

  varUint32(context) {
    let value = 0;
    for (let index = 0; index < 5; index += 1) {
      const byte = this.byte(context);
      if (index === 4 && (byte & 0xf0) !== 0) {
        throw new Error(`${context} exceeds u32`);
      }
      value += (byte & 0x7f) * 2 ** (index * 7);
      if ((byte & 0x80) === 0) {
        return value;
      }
    }
    throw new Error(`${context} has an unterminated u32 LEB128 encoding`);
  }
}

function bytesEqual(left, right) {
  return left.length === right.length && left.every((byte, index) => byte === right[index]);
}

function parseMemoryType(reader, index) {
  const flags = reader.varUint32(`memory ${index} flags`);
  if ((flags & 0x02) !== 0) {
    throw new Error(`memory ${index} is shared; shared memory is not supported`);
  }
  if ((flags & 0x04) !== 0) {
    throw new Error(`memory ${index} is memory64; memory64 is not supported`);
  }
  if ((flags & ~0x07) !== 0) {
    throw new Error(`memory ${index} uses unknown limits flags 0x${flags.toString(16)}`);
  }
  if (flags !== 0 && flags !== 1) {
    throw new Error(`memory ${index} uses unsupported limits flags 0x${flags.toString(16)}`);
  }

  const minimumPages = reader.varUint32(`memory ${index} minimum`);
  const maximumPages = (flags & 1) === 0
    ? null
    : reader.varUint32(`memory ${index} maximum`);
  if (maximumPages !== null && maximumPages < minimumPages) {
    throw new Error(
      `memory ${index} maximum ${maximumPages} is below minimum ${minimumPages}`,
    );
  }
  return { minimum_pages: minimumPages, maximum_pages: maximumPages };
}

export function parseDefinedMemories(input) {
  const bytes = input instanceof Uint8Array ? input : new Uint8Array(input);
  if (bytes.length < WASM_HEADER.length || !bytesEqual(bytes.subarray(0, 8), WASM_HEADER)) {
    throw new Error("invalid Wasm magic or version");
  }

  const reader = new BinaryReader(bytes, WASM_HEADER.length);
  let foundMemorySection = false;
  const memories = [];
  while (reader.offset < reader.end) {
    const sectionId = reader.byte("section id");
    if (sectionId > LAST_KNOWN_SECTION) {
      throw new Error(`unknown Wasm section id ${sectionId}`);
    }
    const payloadLength = reader.varUint32(`section ${sectionId} size`);
    const payloadEnd = reader.offset + payloadLength;
    if (payloadEnd > reader.end) {
      throw new Error(`section ${sectionId} extends beyond the Wasm binary`);
    }

    if (sectionId === MEMORY_SECTION) {
      if (foundMemorySection) {
        throw new Error("duplicate Wasm memory section");
      }
      foundMemorySection = true;
      const memoryReader = new BinaryReader(bytes, reader.offset, payloadEnd);
      const count = memoryReader.varUint32("defined memory count");
      for (let index = 0; index < count; index += 1) {
        memories.push(parseMemoryType(memoryReader, index));
      }
      if (memoryReader.offset !== payloadEnd) {
        throw new Error("memory section contains trailing bytes");
      }
    }
    reader.offset = payloadEnd;
  }
  return memories;
}

function assertAsciiText(value, context) {
  if (!/^[\x20-\x7e]*$/.test(value)) {
    throw new Error(`${context} must contain printable ASCII only`);
  }
}

function compareAsciiText(left, right) {
  return left < right ? -1 : Number(left > right);
}

function compareImport(left, right) {
  return (
    compareAsciiText(left.module, right.module) ||
    compareAsciiText(left.name, right.name) ||
    compareAsciiText(left.kind, right.kind)
  );
}

function compareExport(left, right) {
  return compareAsciiText(left.name, right.name) || compareAsciiText(left.kind, right.kind);
}

function canonicalImports(module) {
  return WebAssembly.Module.imports(module)
    .map(({ module: namespace, name, kind }) => {
      assertAsciiText(namespace, "Wasm import module");
      assertAsciiText(name, "Wasm import name");
      return { module: namespace, name, kind };
    })
    .sort(compareImport);
}

function canonicalExports(module) {
  return WebAssembly.Module.exports(module)
    .map(({ name, kind }) => {
      assertAsciiText(name, "Wasm export name");
      return { name, kind };
    })
    .sort(compareExport);
}

export function compressedSizes(bytes) {
  return {
    raw: bytes.byteLength,
    gzip: gzipSync(bytes, { level: 9 }).byteLength,
    brotli: brotliCompressSync(bytes, {
      params: { [zlibConstants.BROTLI_PARAM_QUALITY]: 11 },
    }).byteLength,
  };
}

export function inspectWasm(input) {
  const bytes = input instanceof Uint8Array ? input : new Uint8Array(input);
  const definedMemories = parseDefinedMemories(bytes);
  let module;
  try {
    module = new WebAssembly.Module(bytes);
  } catch (error) {
    throw new Error(`WebAssembly validation failed: ${error.message}`, { cause: error });
  }
  return {
    imports: canonicalImports(module),
    exports: canonicalExports(module),
    defined_memories: definedMemories,
    sizes: compressedSizes(bytes),
  };
}

function isWasiModule(namespace) {
  return namespace.toLowerCase().startsWith("wasi");
}

export function assertSafeImports(label, imports) {
  for (const entry of imports) {
    if (entry.kind === "memory") {
      throw new Error(
        `${label} imports memory ${entry.module}.${entry.name}; ` +
          "browser artifacts must define independent memories",
      );
    }
    if (entry.module.toLowerCase() === "env" || isWasiModule(entry.module)) {
      throw new Error(
        `${label} has forbidden ${entry.module}.${entry.name} import`,
      );
    }
  }
}

export function assertBridgeImportsResolve(bridgeModule, moonImports, rustExports) {
  const rustExportKeys = new Set(
    rustExports.map(({ name, kind }) => `${kind}\u0000${name}`),
  );
  for (const entry of moonImports) {
    if (entry.module !== bridgeModule) {
      continue;
    }
    if (!rustExportKeys.has(`${entry.kind}\u0000${entry.name}`)) {
      throw new Error(
        `MoonBit bridge import ${entry.name} (${entry.kind}) is not exported by the Rust bridge`,
      );
    }
  }
}

function assertExact(label, field, expected, actual) {
  if (!isDeepStrictEqual(actual, expected)) {
    throw new Error(
      `${label} ${field} changed:\n` +
        `expected ${JSON.stringify(expected, null, 2)}\n` +
        `actual ${JSON.stringify(actual, null, 2)}`,
    );
  }
}

function assertMemoryShape(label, inspection) {
  const imported = inspection.imports.filter(({ kind }) => kind === "memory");
  const exported = inspection.exports.filter(({ kind }) => kind === "memory");
  if (imported.length !== 0 || inspection.defined_memories.length !== 1 || exported.length !== 1) {
    throw new Error(
      `${label} must define and export exactly one memory without importing one`,
    );
  }
}

function assertSizeCaps(label, caps, sizes) {
  for (const encoding of ["raw", "gzip", "brotli"]) {
    const cap = caps?.[encoding];
    if (!Number.isSafeInteger(cap) || cap <= 0) {
      throw new Error(`${label} has an invalid ${encoding} size cap`);
    }
    if (sizes[encoding] > cap) {
      throw new Error(
        `${label} ${encoding} size ${sizes[encoding]} exceeds cap ${cap}`,
      );
    }
  }
}

function assertObservedSizes(label, observedSizes) {
  for (const encoding of ["raw", "gzip", "brotli"]) {
    if (!Number.isSafeInteger(observedSizes?.[encoding]) || observedSizes[encoding] < 0) {
      throw new Error(`${label} has an invalid observed ${encoding} size`);
    }
  }
}

function snapshotEntry(inspection, sizeCaps) {
  return {
    imports: inspection.imports,
    exports: inspection.exports,
    defined_memories: inspection.defined_memories,
    observed_sizes: inspection.sizes,
    size_caps: sizeCaps,
  };
}

export function makeSnapshot(rustInspection, rustGlueSizes, moonInspection) {
  return {
    schema_version: 1,
    bridge_import_module: BRIDGE_IMPORT_MODULE,
    artifacts: {
      rust_bridge: snapshotEntry(rustInspection, DEFAULT_SIZE_CAPS.rust_bridge),
      rust_bridge_glue: {
        observed_sizes: rustGlueSizes,
        size_caps: DEFAULT_SIZE_CAPS.rust_bridge_glue,
      },
      moonbit_canary: snapshotEntry(moonInspection, DEFAULT_SIZE_CAPS.moonbit_canary),
    },
  };
}

export function checkContract(snapshot, rustInspection, rustGlueSizes, moonInspection) {
  if (snapshot?.schema_version !== 1) {
    throw new Error(`unsupported Wasm contract schema ${snapshot?.schema_version}`);
  }
  if (snapshot.bridge_import_module !== BRIDGE_IMPORT_MODULE) {
    throw new Error(
      `Wasm contract bridge_import_module must be ${BRIDGE_IMPORT_MODULE}`,
    );
  }
  const expectedRust = snapshot.artifacts?.rust_bridge;
  const expectedRustGlue = snapshot.artifacts?.rust_bridge_glue;
  const expectedMoon = snapshot.artifacts?.moonbit_canary;
  if (expectedRust === undefined || expectedRustGlue === undefined || expectedMoon === undefined) {
    throw new Error(
      "Wasm contract must describe rust_bridge, rust_bridge_glue, and moonbit_canary",
    );
  }

  assertSafeImports("Rust bridge", rustInspection.imports);
  assertSafeImports("MoonBit canary", moonInspection.imports);
  assertMemoryShape("Rust bridge", rustInspection);
  assertMemoryShape("MoonBit canary", moonInspection);
  assertBridgeImportsResolve(
    snapshot.bridge_import_module,
    moonInspection.imports,
    rustInspection.exports,
  );

  for (const [label, expected, actual] of [
    ["Rust bridge", expectedRust, rustInspection],
    ["MoonBit canary", expectedMoon, moonInspection],
  ]) {
    assertExact(label, "imports", expected.imports, actual.imports);
    assertExact(label, "exports", expected.exports, actual.exports);
    assertExact(
      label,
      "defined memory limits",
      expected.defined_memories,
      actual.defined_memories,
    );
    assertObservedSizes(label, expected.observed_sizes);
    assertSizeCaps(label, expected.size_caps, actual.sizes);
  }
  assertObservedSizes("Rust bridge glue", expectedRustGlue.observed_sizes);
  assertSizeCaps("Rust bridge glue", expectedRustGlue.size_caps, rustGlueSizes);
}

function formatDelta(value) {
  return value >= 0 ? `+${value}` : `${value}`;
}

function formatSizeLine(label, sizes, observedSizes, caps) {
  return `${label}: ${["raw", "gzip", "brotli"]
    .map((encoding) => {
      const delta = sizes[encoding] - observedSizes[encoding];
      return (
        `${encoding} ${sizes[encoding]} bytes ` +
        `(baseline ${observedSizes[encoding]}, delta ${formatDelta(delta)}, ` +
        `cap ${caps[encoding]})`
      );
    })
    .join(", ")}`;
}

async function loadInspection(file) {
  const bytes = await readFile(file);
  try {
    return inspectWasm(bytes);
  } catch (error) {
    throw new Error(`${file}: ${error.message}`, { cause: error });
  }
}

function usage() {
  return (
    "usage: node scripts/check-wasm-contract.mjs [--print-snapshot] " +
    "RUST_BRIDGE.wasm RUST_BRIDGE.mjs MOONBIT.wasm [SNAPSHOT.json]"
  );
}

async function main(arguments_) {
  const printSnapshot = arguments_[0] === "--print-snapshot";
  const paths = printSnapshot ? arguments_.slice(1) : arguments_;
  if ((printSnapshot && paths.length !== 3) || (!printSnapshot && paths.length !== 4)) {
    throw new Error(usage());
  }
  const [rustPath, rustGluePath, moonPath, snapshotPath] = paths;
  const [rustInspection, rustGlue, moonInspection] = await Promise.all([
    loadInspection(rustPath),
    readFile(rustGluePath),
    loadInspection(moonPath),
  ]);
  const rustGlueSizes = compressedSizes(rustGlue);

  if (printSnapshot) {
    process.stdout.write(
      `${JSON.stringify(makeSnapshot(rustInspection, rustGlueSizes, moonInspection), null, 2)}\n`,
    );
    return;
  }

  const snapshot = JSON.parse(await readFile(snapshotPath, "utf8"));
  checkContract(snapshot, rustInspection, rustGlueSizes, moonInspection);
  const { rust_bridge: rust, rust_bridge_glue: glue, moonbit_canary: moon } =
    snapshot.artifacts;
  process.stdout.write(
    `${formatSizeLine("Rust bridge", rustInspection.sizes, rust.observed_sizes, rust.size_caps)}\n`,
  );
  process.stdout.write(
    `${formatSizeLine("Rust bridge glue", rustGlueSizes, glue.observed_sizes, glue.size_caps)}\n`,
  );
  process.stdout.write(
    `${formatSizeLine("MoonBit canary", moonInspection.sizes, moon.observed_sizes, moon.size_caps)}\n`,
  );
  process.stdout.write("Wasm browser-memory contract is unchanged.\n");
}

if (process.argv[1] !== undefined && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main(process.argv.slice(2)).catch((error) => {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = 1;
  });
}
