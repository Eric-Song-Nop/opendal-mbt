import { spawn } from "node:child_process";
import { randomBytes } from "node:crypto";
import { access, mkdtemp, readFile, rm } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const MAX_RESULT_BYTES = 64 * 1024;
const TIMEOUT_MS = 60_000;
const BRIDGE_STEM = "opendal_mbt_browser_bridge";

const canaryDirectory = fileURLToPath(new URL(".", import.meta.url));
const repositoryRoot = resolve(canaryDirectory, "../..");

function usage() {
  return (
    "usage: node wasm/browser-canary/run.mjs " +
    "[target/browser-js/<profile>]"
  );
}

async function findBrowser() {
  if (process.env.OPENDAL_MBT_BROWSER_BIN) {
    await access(process.env.OPENDAL_MBT_BROWSER_BIN);
    return process.env.OPENDAL_MBT_BROWSER_BIN;
  }
  const candidates = [
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    "/usr/bin/google-chrome",
    "/usr/bin/google-chrome-stable",
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
    "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
    "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
  ];
  for (const candidate of candidates) {
    try {
      await access(candidate);
      return candidate;
    } catch {
      // Continue to the next known browser path.
    }
  }
  throw new Error(
    "could not find Chrome/Chromium; set OPENDAL_MBT_BROWSER_BIN to its executable",
  );
}

function listen(server) {
  return new Promise((resolveListen, rejectListen) => {
    server.once("error", rejectListen);
    server.listen(0, "127.0.0.1", () => {
      server.off("error", rejectListen);
      resolveListen(server.address());
    });
  });
}

function closeServer(server) {
  server.closeAllConnections?.();
  return new Promise((resolveClose) => server.close(resolveClose));
}

async function terminate(child) {
  if (child.exitCode !== null || child.signalCode !== null) {
    return;
  }
  const signal = (name) => {
    try {
      if (process.platform !== "win32") {
        process.kill(-child.pid, name);
      } else {
        child.kill(name);
      }
    } catch {
      try {
        child.kill(name);
      } catch {
        // The browser already exited.
      }
    }
  };
  signal("SIGTERM");
  await Promise.race([
    new Promise((resolveExit) => child.once("exit", resolveExit)),
    new Promise((resolveDelay) => setTimeout(resolveDelay, 1_000)),
  ]);
  if (child.exitCode === null && child.signalCode === null) {
    signal("SIGKILL");
  }
}

function responseHeaders(contentType) {
  return {
    "cache-control": "no-store",
    "content-security-policy":
      "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; " +
      "connect-src 'self'; style-src 'none'; img-src 'none'; " +
      "base-uri 'none'; form-action 'none'",
    "content-type": contentType,
    "cross-origin-opener-policy": "same-origin",
    "cross-origin-resource-policy": "same-origin",
    "x-content-type-options": "nosniff",
  };
}

async function main() {
  const [bridgeDirectoryArgument, ...extraArguments] = process.argv.slice(2);
  if (extraArguments.length !== 0) {
    throw new Error(usage());
  }
  const bridgeDirectory = resolve(
    bridgeDirectoryArgument ?? join(repositoryRoot, "target/browser-js/debug"),
  );
  const runtimePath = join(repositoryRoot, "wasm/browser-runtime/index.mjs");
  const gluePath = join(bridgeDirectory, `${BRIDGE_STEM}.mjs`);
  const bridgePath = join(bridgeDirectory, `${BRIDGE_STEM}_bg.wasm`);

  const assets = new Map([
    [
      "/index.html",
      ["text/html; charset=utf-8", await readFile(join(canaryDirectory, "index.html"))],
    ],
    [
      "/browser-page.mjs",
      ["text/javascript; charset=utf-8", await readFile(join(canaryDirectory, "browser-page.mjs"))],
    ],
    [
      "/browser-runtime.mjs",
      ["text/javascript; charset=utf-8", await readFile(runtimePath)],
    ],
    [
      `/${BRIDGE_STEM}.mjs`,
      ["text/javascript; charset=utf-8", await readFile(gluePath)],
    ],
    [
      `/${BRIDGE_STEM}_bg.wasm`,
      ["application/wasm", await readFile(bridgePath)],
    ],
  ]);

  const token = randomBytes(24).toString("hex");
  let acceptResult;
  let rejectResult;
  let resultSettled = false;
  const result = new Promise((resolveResult, reject) => {
    acceptResult = (payload) => {
      if (!resultSettled) {
        resultSettled = true;
        resolveResult(payload);
      }
    };
    rejectResult = (error) => {
      if (!resultSettled) {
        resultSettled = true;
        reject(error);
      }
    };
  });

  const server = createServer((request, response) => {
    const url = new URL(request.url ?? "/", "http://127.0.0.1");
    if (request.method === "POST" && url.pathname === "/result") {
      if (url.searchParams.get("token") !== token) {
        response.writeHead(403).end();
        return;
      }
      const chunks = [];
      let size = 0;
      request.on("data", (chunk) => {
        size += chunk.length;
        if (size > MAX_RESULT_BYTES) {
          request.destroy(new Error("browser result exceeded size limit"));
          return;
        }
        chunks.push(chunk);
      });
      request.on("error", rejectResult);
      request.on("end", () => {
        try {
          const payload = JSON.parse(Buffer.concat(chunks).toString("utf8"));
          if (typeof payload?.ok !== "boolean") {
            throw new Error("browser result did not contain a boolean ok field");
          }
          response.writeHead(204, responseHeaders("text/plain; charset=utf-8"));
          response.end();
          acceptResult(payload);
        } catch (error) {
          response.writeHead(400).end();
          rejectResult(error);
        }
      });
      return;
    }

    if (request.method !== "GET") {
      response.writeHead(405).end();
      return;
    }
    const assetPath = url.pathname === "/" ? "/index.html" : url.pathname;
    const asset = assets.get(assetPath);
    if (!asset) {
      response.writeHead(404).end();
      return;
    }
    response.writeHead(200, responseHeaders(asset[0]));
    response.end(asset[1]);
  });

  const profile = await mkdtemp(join(tmpdir(), "opendal-mbt-browser-js-"));
  let child;
  let timeoutHandle;
  try {
    const address = await listen(server);
    const browser = await findBrowser();
    const url = `http://127.0.0.1:${address.port}/?token=${token}`;
    const browserArguments = [
      "--headless=new",
      "--disable-background-networking",
      "--disable-component-update",
      "--disable-default-apps",
      "--disable-dev-shm-usage",
      "--disable-extensions",
      "--disable-gpu",
      "--no-default-browser-check",
      "--no-first-run",
      `--user-data-dir=${profile}`,
      url,
    ];
    if (typeof process.getuid === "function" && process.getuid() === 0) {
      browserArguments.unshift("--no-sandbox");
    }
    child = spawn(browser, browserArguments, {
      detached: process.platform !== "win32",
      stdio: ["ignore", "ignore", "pipe"],
    });
    let stderr = "";
    child.stderr.setEncoding("utf8");
    child.stderr.on("data", (chunk) => {
      stderr = (stderr + chunk).slice(-32_768);
    });
    const earlyExit = new Promise((_, reject) => {
      child.once("error", reject);
      child.once("exit", (code, signal) => {
        reject(
          new Error(
            `${basename(browser)} exited before reporting ` +
              `(code=${code}, signal=${signal})\n${stderr}`,
          ),
        );
      });
    });
    const timeout = new Promise((_, reject) => {
      timeoutHandle = setTimeout(
        () => reject(new Error("browser JavaScript canary timed out")),
        TIMEOUT_MS,
      );
    });
    const payload = await Promise.race([result, earlyExit, timeout]);
    if (!payload.ok) {
      throw new Error(`browser JavaScript canary failed:\n${payload.error}`);
    }
    process.stdout.write(`${JSON.stringify(payload)}\n`);
  } finally {
    clearTimeout(timeoutHandle);
    if (child) {
      await terminate(child);
    }
    await closeServer(server);
    await rm(profile, { recursive: true, force: true, maxRetries: 3 });
  }
}

await main();
