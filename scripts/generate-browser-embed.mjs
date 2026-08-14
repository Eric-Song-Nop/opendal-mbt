#!/usr/bin/env node

import { createHash } from "node:crypto";
import { mkdir, readFile, rename, unlink, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { gunzipSync, gzipSync } from "node:zlib";

const BASE64_COLUMNS = 72;
const CHUNKS_PER_FUNCTION = 12_000;
const EXPECTED_WASM_BINDGEN_VERSION = "0.2.127";

function fail(message) {
  throw new Error(message);
}

function parseArguments(argv) {
  const values = new Map();
  let check = false;
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--check") {
      check = true;
      continue;
    }
    if (!argument.startsWith("--")) {
      fail(`unexpected argument ${argument}`);
    }
    const value = argv[index + 1];
    if (value === undefined || value.startsWith("--")) {
      fail(`${argument} requires a value`);
    }
    values.set(argument.slice(2), value);
    index += 1;
  }
  const required = ["glue", "wasm", "runtime", "output"];
  for (const name of required) {
    if (!values.has(name)) {
      fail(`--${name} is required`);
    }
  }
  return {
    check,
    glue: resolve(values.get("glue")),
    wasm: resolve(values.get("wasm")),
    runtime: resolve(values.get("runtime")),
    output: resolve(values.get("output")),
    wasmBindgenVersion:
      values.get("wasm-bindgen-version") ?? EXPECTED_WASM_BINDGEN_VERSION,
  };
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function normalizeGzipHeader(bytes) {
  if (
    bytes.length < 10 ||
    bytes[0] !== 0x1f ||
    bytes[1] !== 0x8b ||
    bytes[2] !== 0x08
  ) {
    fail("Node zlib did not produce a gzip stream");
  }
  bytes.fill(0, 4, 8);
  bytes[9] = 0xff;
  return bytes;
}

function validateNormalizedGzipHeader(bytes) {
  if (
    bytes.length < 10 ||
    bytes[0] !== 0x1f ||
    bytes[1] !== 0x8b ||
    bytes[2] !== 0x08 ||
    bytes[3] !== 0 ||
    bytes[4] !== 0 ||
    bytes[5] !== 0 ||
    bytes[6] !== 0 ||
    bytes[7] !== 0 ||
    bytes[9] !== 0xff
  ) {
    fail("checked-in browser payload has a non-canonical gzip header");
  }
}

function compressedPayloadFromGenerated(source) {
  const chunks = Array.from(
    source.matchAll(/^  #\|   "([A-Za-z0-9+/]{1,72}={0,2})",$/gm),
    (match) => match[1],
  );
  if (chunks.length === 0) {
    fail("checked-in browser payload has no base64 chunks");
  }
  const compressed = Buffer.from(chunks.join(""), "base64");
  validateNormalizedGzipHeader(compressed);
  return compressed;
}

function stripRuntimeExports(source) {
  const transformed = source
    .replace(/^export class /gm, "class ")
    .replace(/^export async function /gm, "async function ")
    .replace(/^export function /gm, "function ");
  if (/^\s*(?:export|import)\b/m.test(transformed) || /import\.meta/.test(transformed)) {
    fail("browser Promise runtime retains unsupported module syntax");
  }
  return transformed;
}

function validateNoModulesGlue(source) {
  if (!source.startsWith("let wasm_bindgen = (function(exports)")) {
    fail("wasm-bindgen glue was not generated with --target no-modules");
  }
  if (/^\s*(?:export|import)\b/m.test(source) || /import\.meta/.test(source)) {
    fail("wasm-bindgen no-modules glue contains module syntax");
  }
}

function moonJsLines(source) {
  if (source.includes("#|")) {
    fail("embedded JavaScript contains MoonBit multiline-string syntax");
  }
  return source
    .split("\n")
    .map((line) => `  #| ${line}`)
    .join("\n");
}

function chunkText(source, width) {
  const chunks = [];
  for (let offset = 0; offset < source.length; offset += width) {
    chunks.push(source.slice(offset, offset + width));
  }
  return chunks;
}

function renderPartFunction(chunks, index) {
  return `///|
extern "js" fn embedded_wasm_part_${index}() -> String =
  #| () => [
${chunks.map((chunk) => `  #|   "${chunk}",`).join("\n")}
  #| ].join("")
`;
}

function extractRuntimeContract(runtime) {
  const abi = runtime.match(/^const ABI_VERSION = (0x[0-9a-f_]+);$/m)?.[1];
  const features = runtime.match(
    /^const REQUIRED_FEATURE_FLAGS = (0x[0-9a-f_]+);$/m,
  )?.[1];
  if (!abi || !features) {
    fail("browser Promise runtime does not declare its ABI contract");
  }
  return { abi, features };
}

function renderGeneratedSource({
  glue,
  wasm,
  compressed,
  runtime,
  wasmBindgenVersion,
}) {
  validateNoModulesGlue(glue);
  const strippedRuntime = stripRuntimeExports(runtime);
  const contract = extractRuntimeContract(runtime);
  const encoded = compressed.toString("base64");
  const chunks = chunkText(encoded, BASE64_COLUMNS);
  const parts = [];
  for (let offset = 0; offset < chunks.length; offset += CHUNKS_PER_FUNCTION) {
    parts.push(chunks.slice(offset, offset + CHUNKS_PER_FUNCTION));
  }
  if (parts.length === 0) {
    fail("browser bridge Wasm payload is empty");
  }
  const partFunctions = parts
    .map((part, index) => renderPartFunction(part, index))
    .join("\n");
  const partCalls = parts
    .map((_, index) => `embedded_wasm_part_${index}()`)
    .join(", ");

  return `// Code generated by scripts/generate-browser-embed.mjs; DO NOT EDIT.
// wasm-bindgen ${wasmBindgenVersion}; ABI ${contract.abi}; features ${contract.features}
// glue sha256 ${sha256(glue)}
// wasm sha256 ${sha256(wasm)}; gzip sha256 ${sha256(compressed)}
// runtime sha256 ${sha256(runtime)}

${partFunctions}
///|
extern "js" fn load_embedded_js_runtime(
  parts : Array[String],
) -> @js_async.Promise[JsRuntime] =
  #| async (parts) => {
  #|   if (typeof DecompressionStream !== "function") {
  #|     throw new Error("embedded OpenDAL requires browser DecompressionStream support")
  #|   }
  #|   const encoded = parts.join("")
  #|   const binary = atob(encoded)
  #|   const compressed = Uint8Array.from(binary, (byte) => byte.charCodeAt(0))
  #|   const stream = new Blob([compressed])
  #|     .stream()
  #|     .pipeThrough(new DecompressionStream("gzip"))
  #|   const wasmBytes = new Uint8Array(await new Response(stream).arrayBuffer())
${moonJsLines(glue)}
${moonJsLines(strippedRuntime)}
  #|   const bridge = await wasm_bindgen({ module_or_path: wasmBytes })
  #|   return loadOpenDalBrowser({ bridge })
  #| }

///|
fn embedded_runtime_parts() -> Array[String] {
  [${partCalls}]
}
`;
}

async function writeAtomically(filename, contents) {
  await mkdir(dirname(filename), { recursive: true });
  const temporary = `${filename}.tmp-${process.pid}`;
  try {
    await writeFile(temporary, contents);
    await rename(temporary, filename);
  } finally {
    await unlink(temporary).catch((error) => {
      if (error?.code !== "ENOENT") throw error;
    });
  }
}

async function main() {
  const options = parseArguments(process.argv.slice(2));
  if (options.wasmBindgenVersion !== EXPECTED_WASM_BINDGEN_VERSION) {
    fail(
      `expected wasm-bindgen ${EXPECTED_WASM_BINDGEN_VERSION}, got ${options.wasmBindgenVersion}`,
    );
  }
  const [glue, wasm, runtime] = await Promise.all([
    readFile(options.glue, "utf8"),
    readFile(options.wasm),
    readFile(options.runtime, "utf8"),
  ]);
  let existing;
  let compressed;
  if (options.check) {
    existing = await readFile(options.output, "utf8").catch((error) => {
      if (error?.code === "ENOENT") return undefined;
      throw error;
    });
    if (existing === undefined) {
      fail(
        `${options.output} is missing; run make browser-embed-generate and commit the result`,
      );
    }
    // Reuse the committed deflate stream after proving its decompressed bytes.
    // zlib versions may choose different valid deflate encodings; this keeps
    // --check semantic across platforms while the full generated layout,
    // glue, runtime, hashes, and canonical gzip header remain exact.
    compressed = compressedPayloadFromGenerated(existing);
    let decompressed;
    try {
      decompressed = gunzipSync(compressed);
    } catch (error) {
      fail(`checked-in browser payload is not valid gzip: ${error.message}`);
    }
    if (!decompressed.equals(wasm)) {
      fail("checked-in browser payload does not contain the current bridge Wasm");
    }
  } else {
    compressed = normalizeGzipHeader(gzipSync(wasm, { level: 9 }));
  }
  const generated = renderGeneratedSource({
    glue,
    wasm,
    compressed,
    runtime,
    wasmBindgenVersion: options.wasmBindgenVersion,
  });
  if (options.check) {
    if (existing !== generated) {
      fail(
        `${options.output} is stale; run make browser-embed-generate and commit the result`,
      );
    }
    return;
  }
  await writeAtomically(options.output, generated);
}

main().catch((error) => {
  console.error(`generate-browser-embed: ${error.message}`);
  process.exitCode = 1;
});
