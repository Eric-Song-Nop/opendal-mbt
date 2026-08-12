const DEFAULT_IMPORT_MODULE = "opendal_mbt_bridge";

function isResponse(value) {
  return typeof Response !== "undefined" && value instanceof Response;
}

async function toModule(source) {
  if (source instanceof WebAssembly.Module) {
    return source;
  }
  if (isResponse(source)) {
    if (!source.ok) {
      throw new Error(`could not fetch Wasm module: HTTP ${source.status}`);
    }
    const fallback = source.clone();
    try {
      return await WebAssembly.compileStreaming(Promise.resolve(source));
    } catch {
      return WebAssembly.compile(await fallback.arrayBuffer());
    }
  }
  if (source instanceof URL && source.protocol === "file:") {
    if (typeof process === "undefined" || !process.versions?.node) {
      throw new TypeError("file: Wasm URLs are only supported by the Node loader");
    }
    const { readFile } = await import("node:fs/promises");
    return WebAssembly.compile(await readFile(source));
  }
  if (source instanceof URL || typeof source === "string") {
    return toModule(await fetch(source));
  }
  if (source instanceof ArrayBuffer || ArrayBuffer.isView(source)) {
    return WebAssembly.compile(source);
  }
  throw new TypeError(
    "Wasm source must be a URL, Response, WebAssembly.Module, ArrayBuffer, or typed array",
  );
}

/**
 * Load the Rust OpenDAL bridge and a MoonBit Wasm consumer wired to it.
 *
 * The consumer only imports scalar bridge functions. Rust and MoonBit retain
 * separate memories and allocators; the MoonBit facade performs explicit
 * byte copies through generation-checked bridge handles.
 */
export async function loadOpenDalMoonBit({
  bridge,
  moonbit,
  imports = {},
  bridgeImports = {},
  importModule = DEFAULT_IMPORT_MODULE,
}) {
  const bridgeModule = await toModule(bridge);
  const bridgeInstance = await WebAssembly.instantiate(
    bridgeModule,
    bridgeImports,
  );
  const moonbitModule = await toModule(moonbit);
  const existing = imports[importModule] ?? {};
  const moonbitImports = {
    ...imports,
    [importModule]: {
      ...existing,
      ...bridgeInstance.exports,
    },
  };
  const moonbitInstance = await WebAssembly.instantiate(
    moonbitModule,
    moonbitImports,
  );
  return {
    exports: moonbitInstance.exports,
    bridge: bridgeInstance,
    moonbit: moonbitInstance,
  };
}

export default loadOpenDalMoonBit;
