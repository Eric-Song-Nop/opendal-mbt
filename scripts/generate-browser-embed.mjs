#!/usr/bin/env node

import { createHash } from "node:crypto";
import {
  mkdir,
  readFile,
  readdir,
  rename,
  unlink,
  writeFile,
} from "node:fs/promises";
import { dirname, isAbsolute, resolve } from "node:path";
import { gunzipSync, gzipSync } from "node:zlib";

const BASE64_COLUMNS = 72;
const CHUNKS_PER_FUNCTION = 12_000;
const EXPECTED_WASM_BINDGEN_VERSION = "0.2.127";
const GLUE_START = "  #| // opendal-mbt:wasm-bindgen-glue:start";
const GLUE_END = "  #| // opendal-mbt:wasm-bindgen-glue:end";

function fail(message) {
  throw new Error(message);
}

function compareText(left, right) {
  return left < right ? -1 : left > right ? 1 : 0;
}

function parseArguments(argv) {
  const values = new Map();
  const sources = [];
  const sourceDirectories = [];
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
    const name = argument.slice(2);
    if (name === "source" || name === "source-dir") {
      const label = value.replaceAll("\\", "/").replace(/^\.\//, "");
      if (
        isAbsolute(value) ||
        label.length === 0 ||
        label.split("/").includes("..")
      ) {
        fail(`--${name} must be a workspace-relative path, got ${value}`);
      }
      const destination = name === "source" ? sources : sourceDirectories;
      destination.push({ label, path: resolve(value) });
    } else {
      values.set(name, value);
    }
    index += 1;
  }
  const required = ["glue", "wasm", "runtime", "output"];
  for (const name of required) {
    if (!values.has(name)) {
      fail(`--${name} is required`);
    }
  }
  if (sources.length === 0 && sourceDirectories.length === 0) {
    fail("at least one --source or --source-dir is required");
  }
  sources.sort((left, right) => compareText(left.label, right.label));
  for (let index = 1; index < sources.length; index += 1) {
    if (sources[index - 1].label === sources[index].label) {
      fail(`duplicate --source ${sources[index].label}`);
    }
  }
  return {
    check,
    glue: resolve(values.get("glue")),
    wasm: resolve(values.get("wasm")),
    runtime: resolve(values.get("runtime")),
    output: resolve(values.get("output")),
    sources,
    sourceDirectories,
    wasmBindgenVersion:
      values.get("wasm-bindgen-version") ?? EXPECTED_WASM_BINDGEN_VERSION,
  };
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function sourceFingerprint(sources) {
  const digest = createHash("sha256");
  for (const source of sources) {
    const contents = Buffer.from(
      source.contents.toString("utf8").replaceAll("\r\n", "\n"),
    );
    digest.update(source.label);
    digest.update("\0");
    digest.update(String(contents.length));
    digest.update("\0");
    digest.update(contents);
    digest.update("\0");
  }
  return digest.digest("hex");
}

async function readSourceInputs(files, directories) {
  const collected = [...files];
  async function walk(directory) {
    const entries = await readdir(directory.path, { withFileTypes: true });
    entries.sort((left, right) => compareText(left.name, right.name));
    for (const entry of entries) {
      const child = {
        label: `${directory.label}/${entry.name}`,
        path: resolve(directory.path, entry.name),
      };
      if (entry.isDirectory()) {
        await walk(child);
      } else if (entry.isFile()) {
        collected.push(child);
      } else {
        fail(`source fingerprint does not accept ${child.label}`);
      }
    }
  }
  for (const directory of directories) {
    await walk(directory);
  }
  collected.sort((left, right) => compareText(left.label, right.label));
  for (let index = 1; index < collected.length; index += 1) {
    if (collected[index - 1].label === collected[index].label) {
      fail(`duplicate source fingerprint input ${collected[index].label}`);
    }
  }
  return Promise.all(
    collected.map(async (source) => ({
      ...source,
      contents: await readFile(source.path),
    })),
  );
}

function wasmInterface(bytes, description) {
  let module;
  try {
    module = new WebAssembly.Module(bytes);
  } catch (error) {
    fail(`${description} is not a valid WebAssembly module: ${error.message}`);
  }
  const exports = WebAssembly.Module.exports(module)
    .filter(
      (entry) =>
        entry.name === "memory" || entry.name.startsWith("opendal_mbt_wasm_"),
    )
    .sort((left, right) => compareText(left.name, right.name));
  return JSON.stringify(exports);
}

function validateMatchingWasmInterface(embedded, current) {
  if (
    wasmInterface(embedded, "checked-in browser payload") !==
    wasmInterface(current, "current browser bridge")
  ) {
    fail("checked-in browser payload does not expose the current bridge ABI");
  }
}

function embeddedGlueFromGenerated(source) {
  const start = source.indexOf(`${GLUE_START}\n`);
  const end = source.indexOf(`\n${GLUE_END}`);
  if (start === -1 || end === -1 || end <= start) {
    fail("checked-in browser source has no embedded wasm-bindgen glue snapshot");
  }
  const block = source.slice(start + GLUE_START.length + 1, end);
  return block
    .split("\n")
    .map((line) => {
      if (!line.startsWith("  #| ")) {
        fail("checked-in wasm-bindgen glue has malformed MoonBit quoting");
      }
      return line.slice(5);
    })
    .join("\n");
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
  sourcesHash,
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
// sources sha256 ${sourcesHash}

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
${GLUE_START}
${moonJsLines(glue)}
${GLUE_END}
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
  const [glue, wasm, runtime, sources] = await Promise.all([
    readFile(options.glue, "utf8"),
    readFile(options.wasm),
    readFile(options.runtime, "utf8"),
    readSourceInputs(options.sources, options.sourceDirectories),
  ]);
  let existing;
  let compressed;
  let embeddedWasm = wasm;
  let embeddedGlue = glue;
  validateNoModulesGlue(glue);
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
    embeddedGlue = embeddedGlueFromGenerated(existing);
    // Reuse the committed deflate stream after proving its decompressed bytes.
    // zlib versions may choose different valid deflate encodings; this keeps
    // --check semantic across platforms while the full generated layout,
    // glue, runtime, hashes, and canonical gzip header remain exact.
    compressed = compressedPayloadFromGenerated(existing);
    try {
      embeddedWasm = gunzipSync(compressed);
    } catch (error) {
      fail(`checked-in browser payload is not valid gzip: ${error.message}`);
    }
    // Rust/LLVM does not promise byte-identical Wasm across maintainer host
    // architectures. The source fingerprint makes drift explicit, while the
    // ABI comparison and Chrome tests validate both the rebuilt and embedded
    // modules independently.
    validateMatchingWasmInterface(embeddedWasm, wasm);
    if (!embeddedWasm.equals(wasm)) {
      console.log(
        "generate-browser-embed: host-specific bridge bytes differ; " +
          "source fingerprint and public ABI match",
      );
    }
  } else {
    compressed = normalizeGzipHeader(gzipSync(wasm, { level: 9 }));
  }
  const generated = renderGeneratedSource({
    glue: embeddedGlue,
    wasm: embeddedWasm,
    compressed,
    runtime,
    sourcesHash: sourceFingerprint(sources),
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
