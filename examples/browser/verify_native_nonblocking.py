#!/usr/bin/env python3
"""Run the shared delayed-I/O proof with the native OpenDAL backend."""

from __future__ import annotations

import os
from pathlib import Path
import subprocess
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


DELAY_SECONDS = 0.75
EXPECTED_PATH = "/probe-bucket/delayed-missing.txt"
SUCCESS_MARKER = "OPENDAL_ASYNC_IO_NONBLOCKING_OK:"


class DelayedS3Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def do_GET(self) -> None:  # noqa: N802 - HTTP handler API
        if self.path != EXPECTED_PATH:
            body = b"unexpected S3 request path"
            self.send_response(400)
            self.send_header("Content-Type", "text/plain; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        time.sleep(DELAY_SECONDS)
        body = (
            b'<?xml version="1.0" encoding="UTF-8"?>'
            b"<Error><Code>NoSuchKey</Code><Message>missing probe object</Message>"
            b"<Key>delayed-missing.txt</Key><RequestId>probe-request</RequestId>"
            b"<HostId>probe-host</HostId></Error>"
        )
        self.send_response(404)
        self.send_header("Content-Type", "application/xml")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("x-amz-request-id", "probe-request")
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format: str, *args: object) -> None:
        del format, args


def main() -> int:
    example_dir = Path(__file__).resolve().parent
    repository_root = example_dir.parents[1]
    server = ThreadingHTTPServer(("127.0.0.1", 0), DelayedS3Handler)
    server_thread = threading.Thread(target=server.serve_forever, daemon=True)
    server_thread.start()

    environment = os.environ.copy()
    environment["OPENDAL_MBT_PROBE_ENDPOINT"] = (
        f"http://127.0.0.1:{server.server_address[1]}"
    )
    environment["NO_PROXY"] = "127.0.0.1,localhost"
    environment["no_proxy"] = "127.0.0.1,localhost"
    maintainer_library = repository_root / "target/debug/libopendal_mbt_native.a"
    if (
        "OPENDAL_MBT_NATIVE_LIB" not in environment
        and maintainer_library.is_file()
    ):
        environment["OPENDAL_MBT_NATIVE_LIB"] = str(maintainer_library)
        environment["OPENDAL_MBT_SOURCE_PROFILE"] = "standard"
    command = [
        "moon",
        "-C",
        str(example_dir),
        "run",
        "--target",
        "native",
        "--release",
        ".",
    ]
    try:
        completed = subprocess.run(
            command,
            cwd=repository_root,
            env=environment,
            check=False,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=60,
        )
    except subprocess.TimeoutExpired:
        print("native shared non-blocking proof timed out after 60 seconds", file=sys.stderr)
        return 1
    finally:
        server.shutdown()
        server.server_close()
        server_thread.join(timeout=5)

    if completed.stdout:
        print(completed.stdout, end="")
    if completed.stderr:
        print(completed.stderr, end="", file=sys.stderr)
    if completed.returncode != 0:
        return completed.returncode
    if SUCCESS_MARKER not in completed.stdout:
        print("native shared non-blocking proof did not print its success marker", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
