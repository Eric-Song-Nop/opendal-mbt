///|
extern "js" fn is_node() -> Bool =
  #| () => typeof process !== "undefined" && process.versions?.node !== undefined

///|
extern "js" fn report_node_failure(message : String) -> Unit =
  #| (message) => {
  #|   console.error(message)
  #|   process.exitCode = 1
  #| }

///|
extern "js" fn install_browser_failure_reporter() -> Unit =
  #| () => {
  #|   const report = (message) => {
  #|     if (globalThis.__OPENDAL_MBT_REPORTED__) return
  #|     globalThis.__OPENDAL_MBT_REPORTED__ = true
  #|     const url = globalThis.__OPENDAL_MBT_REPORT_URL__
  #|     if (typeof url === "string") {
  #|       void fetch(url, { method: "POST", body: `opendal browser demo failed: ${message}` })
  #|     }
  #|   }
  #|   addEventListener("error", (event) => report(event.message || "page error"))
  #|   addEventListener("unhandledrejection", (event) => {
  #|     report(event.reason?.message || String(event.reason || "unhandled rejection"))
  #|   })
  #| }

///|
extern "js" fn report_from_browser(result : String) -> @js_async.Promise[Unit] =
  #| async (result) => {
  #|   if (globalThis.__OPENDAL_MBT_REPORTED__) return
  #|   globalThis.__OPENDAL_MBT_REPORTED__ = true
  #|   const url = globalThis.__OPENDAL_MBT_REPORT_URL__
  #|   if (typeof url !== "string") throw new Error("browser report URL is missing")
  #|   const response = await fetch(url, { method: "POST", body: result })
  #|   if (!response.ok) throw new Error(`browser report failed: HTTP ${response.status}`)
  #| }

///|
extern "js" fn launch_chrome() -> @js_async.Promise[String] =
  #| async () => {
  #|   const [{ spawn }, crypto, fs, fsPromises, http, os, path] = await Promise.all([
  #|     import("node:child_process"),
  #|     import("node:crypto"),
  #|     import("node:fs"),
  #|     import("node:fs/promises"),
  #|     import("node:http"),
  #|     import("node:os"),
  #|     import("node:path"),
  #|   ])
  #|   const executableSource = process.argv[1]
  #|   if (!executableSource) throw new Error("Moon did not provide the executable path")
  #|   const appSource = await fsPromises.readFile(executableSource, "utf8")
  #|   const token = crypto.randomBytes(24).toString("hex")
  #|   const sockets = new Set()
  #|   let finishResult
  #|   const resultPromise = new Promise((resolve) => { finishResult = resolve })
  #|   const server = http.createServer((request, response) => {
  #|     const requestUrl = new URL(request.url || "/", "http://127.0.0.1")
  #|     if (request.method === "GET" && requestUrl.pathname === "/") {
  #|       const reportUrl = `/result/${token}`
  #|       const html = `<!doctype html><meta charset="utf-8"><title>OpenDAL MoonBit browser demo</title><script>globalThis.__OPENDAL_MBT_REPORT_URL__=${JSON.stringify(reportUrl)}</script><script src="/app.js"></script>`
  #|       response.writeHead(200, {
  #|         "content-type": "text/html; charset=utf-8",
  #|         "content-length": Buffer.byteLength(html),
  #|         "cache-control": "no-store",
  #|         "content-security-policy": "default-src 'none'; script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'; connect-src 'self'",
  #|       })
  #|       response.end(html)
  #|       return
  #|     }
  #|     if (request.method === "GET" && requestUrl.pathname === "/app.js") {
  #|       response.writeHead(200, {
  #|         "content-type": "text/javascript; charset=utf-8",
  #|         "content-length": Buffer.byteLength(appSource),
  #|         "cache-control": "no-store",
  #|       })
  #|       response.end(appSource)
  #|       return
  #|     }
  #|     if (request.method === "POST" && requestUrl.pathname === `/result/${token}`) {
  #|       const chunks = []
  #|       let length = 0
  #|       request.on("data", (chunk) => {
  #|         length += chunk.length
  #|         if (length <= 16 * 1024) chunks.push(chunk)
  #|       })
  #|       request.on("end", () => {
  #|         if (length > 16 * 1024) {
  #|           response.writeHead(413).end()
  #|           return
  #|         }
  #|         const result = Buffer.concat(chunks).toString("utf8")
  #|         response.writeHead(204).end()
  #|         finishResult(result)
  #|       })
  #|       return
  #|     }
  #|     response.writeHead(404).end()
  #|   })
  #|   server.on("connection", (socket) => {
  #|     sockets.add(socket)
  #|     socket.on("close", () => sockets.delete(socket))
  #|   })
  #|   const listen = () => new Promise((resolve, reject) => {
  #|     const onError = (error) => { server.off("listening", onListening); reject(error) }
  #|     const onListening = () => { server.off("error", onError); resolve() }
  #|     server.once("error", onError)
  #|     server.once("listening", onListening)
  #|     server.listen(0, "127.0.0.1")
  #|   })
  #|   const closeServer = async () => {
  #|     if (!server.listening) return
  #|     server.closeAllConnections?.()
  #|     for (const socket of sockets) socket.destroy()
  #|     await new Promise((resolve) => server.close(resolve))
  #|   }
  #|   const canAccess = async (candidate) => {
  #|     if (!candidate) return false
  #|     try {
  #|       await fsPromises.access(candidate, fs.constants.F_OK)
  #|       return true
  #|     } catch {
  #|       return false
  #|     }
  #|   }
  #|   const chromeCandidates = () => {
  #|     const candidates = [process.env.CHROME_BIN]
  #|     if (process.platform === "darwin") {
  #|       candidates.push(
  #|         "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  #|         "/Applications/Chromium.app/Contents/MacOS/Chromium",
  #|       )
  #|     }
  #|     if (process.platform === "win32") {
  #|       for (const base of [
  #|         process.env.PROGRAMFILES,
  #|         process.env["PROGRAMFILES(X86)"],
  #|         process.env.LOCALAPPDATA,
  #|       ]) {
  #|         if (base) candidates.push(path.join(base, "Google", "Chrome", "Application", "chrome.exe"))
  #|       }
  #|     }
  #|     if (process.platform === "linux") {
  #|       candidates.push(
  #|         "/opt/google/chrome/chrome",
  #|         "/opt/google/chrome/google-chrome",
  #|         "/usr/lib/chromium/chromium",
  #|         "/usr/lib/chromium-browser/chromium-browser",
  #|         "/usr/bin/google-chrome-stable",
  #|         "/usr/bin/google-chrome",
  #|         "/usr/bin/chromium",
  #|         "/snap/bin/chromium",
  #|       )
  #|     }
  #|     const pathNames = process.platform === "win32"
  #|       ? ["chrome.exe", "msedge.exe"]
  #|       : ["google-chrome-stable", "google-chrome", "chromium", "chromium-browser"]
  #|     for (const directory of (process.env.PATH || "").split(path.delimiter)) {
  #|       if (!directory) continue
  #|       for (const name of pathNames) candidates.push(path.join(directory, name))
  #|     }
  #|     return candidates
  #|   }
  #|   let chrome
  #|   for (const candidate of chromeCandidates()) {
  #|     if (await canAccess(candidate)) {
  #|       chrome = candidate
  #|       break
  #|     }
  #|   }
  #|   if (!chrome) {
  #|     throw new Error("Chrome or Chromium was not found; set CHROME_BIN")
  #|   }
  #|   const profile = await fsPromises.mkdtemp(path.join(os.tmpdir(), "opendal-mbt-browser-"))
  #|   let child
  #|   const waitForExit = (processHandle, timeout) => {
  #|     if (processHandle.exitCode !== null || processHandle.signalCode !== null) {
  #|       return Promise.resolve(true)
  #|     }
  #|     return new Promise((resolve) => {
  #|       let timer
  #|       const finish = (exited) => {
  #|         processHandle.off("exit", onExit)
  #|         processHandle.off("error", onError)
  #|         clearTimeout(timer)
  #|         resolve(exited)
  #|       }
  #|       const onExit = () => finish(true)
  #|       const onError = () => finish(true)
  #|       processHandle.once("exit", onExit)
  #|       processHandle.once("error", onError)
  #|       timer = setTimeout(() => finish(false), timeout)
  #|     })
  #|   }
  #|   const stopChild = async () => {
  #|     if (!child || child.exitCode !== null || child.signalCode !== null) return
  #|     if (process.platform === "win32") {
  #|       const killer = spawn("taskkill", ["/pid", String(child.pid), "/T", "/F"], {
  #|         stdio: "ignore",
  #|         windowsHide: true,
  #|       })
  #|       await waitForExit(killer, 5000)
  #|       await waitForExit(child, 1500)
  #|       return
  #|     }
  #|     try { child.kill("SIGTERM") } catch {}
  #|     if (!(await waitForExit(child, 1500))) {
  #|       try { child.kill("SIGKILL") } catch {}
  #|       await waitForExit(child, 1500)
  #|     }
  #|   }
  #|   try {
  #|     await listen()
  #|     const address = server.address()
  #|     if (!address || typeof address === "string") throw new Error("HTTP server has no port")
  #|     const url = `http://127.0.0.1:${address.port}/`
  #|     const args = [
  #|       "--headless=new",
  #|       "--disable-background-networking",
  #|       "--disable-component-update",
  #|       "--disable-default-apps",
  #|       "--disable-gpu",
  #|       "--disable-sync",
  #|       "--metrics-recording-only",
  #|       "--mute-audio",
  #|       "--no-default-browser-check",
  #|       "--no-first-run",
  #|       `--user-data-dir=${profile}`,
  #|     ]
  #|     if (process.platform === "linux") {
  #|       args.push("--disable-dev-shm-usage")
  #|       const isCi = process.env.CI === "true" || process.env.GITHUB_ACTIONS === "true"
  #|       if (isCi || process.getuid?.() === 0) args.push("--no-sandbox")
  #|     }
  #|     args.push(url)
  #|     child = spawn(chrome, args, { stdio: ["ignore", "ignore", "pipe"], windowsHide: true })
  #|     let stderr = ""
  #|     child.stderr.on("data", (chunk) => {
  #|       if (stderr.length < 16 * 1024) stderr += chunk.toString("utf8")
  #|     })
  #|     const childOutcome = new Promise((resolve) => {
  #|       child.once("error", (error) => resolve({ kind: "error", error }))
  #|       child.once("exit", (code, signal) => resolve({ kind: "exit", code, signal }))
  #|     })
  #|     let timeoutHandle
  #|     const timeoutOutcome = new Promise((resolve) => {
  #|       timeoutHandle = setTimeout(() => resolve({ kind: "timeout" }), 45_000)
  #|     })
  #|     const outcome = await Promise.race([
  #|       resultPromise.then((value) => ({ kind: "result", value })),
  #|       childOutcome,
  #|       timeoutOutcome,
  #|     ])
  #|     clearTimeout(timeoutHandle)
  #|     if (outcome.kind === "error") throw outcome.error
  #|     if (outcome.kind === "exit") {
  #|       throw new Error(`Chrome exited before reporting (code=${outcome.code}, signal=${outcome.signal})\n${stderr}`)
  #|     }
  #|     if (outcome.kind === "timeout") throw new Error(`Chrome did not report within 45 seconds\n${stderr}`)
  #|     if (!outcome.value.startsWith("opendal browser demo ok:")) {
  #|       throw new Error(outcome.value || "Chrome reported an empty result")
  #|     }
  #|     return outcome.value
  #|   } finally {
  #|     await stopChild()
  #|     await closeServer()
  #|     await fsPromises.rm(profile, { recursive: true, force: true, maxRetries: 8, retryDelay: 100 })
  #|   }
  #| }
