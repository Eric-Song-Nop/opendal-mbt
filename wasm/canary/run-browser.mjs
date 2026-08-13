import { spawn } from "node:child_process";
import { randomBytes } from "node:crypto";
import { access, mkdtemp, readFile, rm } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";

const MAX_RESULT_BYTES = 64 * 1024;
const TIMEOUT_MS = 30_000;

function usage() {
  return (
    "usage: node wasm/canary/run-browser.mjs " +
    "<rust-bridge.mjs> <rust-bridge.wasm> <moonbit-canary.wasm>"
  );
}

async function findBrowser() {
  if (process.env.OPENDAL_MBT_BROWSER_BIN) {
    return process.env.OPENDAL_MBT_BROWSER_BIN;
  }
  const candidates = [
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/usr/bin/google-chrome",
    "/usr/bin/google-chrome-stable",
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
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
  return new Promise((resolveListen, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      server.off("error", reject);
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
      process.kill(-child.pid, name);
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

async function main() {
  const [gluePath, bridgePath, moonbitPath] = process.argv.slice(2);
  if (!gluePath || !bridgePath || !moonbitPath) {
    throw new Error(usage());
  }

  const token = randomBytes(24).toString("hex");
  const assets = new Map([
    ["/browser-page.mjs", ["text/javascript", await readFile(new URL("./browser-page.mjs", import.meta.url))]],
    ["/loader.mjs", ["text/javascript", await readFile(new URL("../loader/index.mjs", import.meta.url))]],
    ["/bridge.mjs", ["text/javascript", await readFile(resolve(gluePath))]],
    ["/bridge.wasm", ["application/wasm", await readFile(resolve(bridgePath))]],
    ["/moonbit.wasm", ["application/wasm", await readFile(resolve(moonbitPath))]],
  ]);

  let acceptResult;
  let rejectResult;
  const result = new Promise((resolveResult, reject) => {
    acceptResult = resolveResult;
    rejectResult = reject;
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
          response.writeHead(204).end();
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
    if (url.pathname === "/") {
      response.writeHead(200, {
        "content-type": "text/html; charset=utf-8",
        "cache-control": "no-store",
      });
      response.end('<!doctype html><script type="module" src="/browser-page.mjs"></script>');
      return;
    }
    const asset = assets.get(url.pathname);
    if (!asset) {
      response.writeHead(404).end();
      return;
    }
    response.writeHead(200, {
      "content-type": asset[0],
      "cache-control": "no-store",
    });
    response.end(asset[1]);
  });

  const profile = await mkdtemp(join(tmpdir(), "opendal-mbt-chrome-"));
  let child;
  let timeoutHandle;
  try {
    const address = await listen(server);
    const browser = await findBrowser();
    const url = `http://127.0.0.1:${address.port}/?token=${token}`;
    child = spawn(
      browser,
      [
        "--headless=new",
        "--disable-gpu",
        "--no-first-run",
        "--no-default-browser-check",
        `--user-data-dir=${profile}`,
        url,
      ],
      { detached: true, stdio: ["ignore", "ignore", "pipe"] },
    );
    let stderr = "";
    child.stderr.setEncoding("utf8");
    child.stderr.on("data", (chunk) => {
      stderr = (stderr + chunk).slice(-16_384);
    });
    const earlyExit = new Promise((_, reject) => {
      child.once("error", reject);
      child.once("exit", (code, signal) => {
        reject(
          new Error(
            `${basename(browser)} exited before reporting (code=${code}, signal=${signal})\n${stderr}`,
          ),
        );
      });
    });
    const timeout = new Promise((_, reject) => {
      timeoutHandle = setTimeout(
        () => reject(new Error("browser canary timed out")),
        TIMEOUT_MS,
      );
    });
    const payload = await Promise.race([result, earlyExit, timeout]);
    if (!payload.ok) {
      throw new Error(`browser canary failed:\n${payload.error}`);
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
