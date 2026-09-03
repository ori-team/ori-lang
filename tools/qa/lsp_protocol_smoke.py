#!/usr/bin/env python3
"""Minimal protocol smoke for a packaged ori-lsp binary."""

from __future__ import annotations

import json
import subprocess
import sys
import time


def send(process: subprocess.Popen[bytes], message: dict) -> None:
    payload = json.dumps(message, separators=(",", ":")).encode("utf-8")
    process.stdin.write(f"Content-Length: {len(payload)}\r\n\r\n".encode() + payload)
    process.stdin.flush()


def receive(process: subprocess.Popen[bytes], timeout: float = 10.0) -> dict:
    deadline = time.monotonic() + timeout
    headers = b""
    while b"\r\n\r\n" not in headers:
        if time.monotonic() > deadline:
            raise RuntimeError("timed out waiting for LSP headers")
        byte = process.stdout.read(1)
        if not byte:
            raise RuntimeError("ori-lsp exited before responding")
        headers += byte
    header_text, _ = headers.split(b"\r\n\r\n", 1)
    length = None
    for line in header_text.decode("ascii").split("\r\n"):
        if line.lower().startswith("content-length:"):
            length = int(line.split(":", 1)[1].strip())
    if length is None:
        raise RuntimeError("LSP response omitted Content-Length")
    body = process.stdout.read(length)
    if len(body) != length:
        raise RuntimeError("truncated LSP response")
    return json.loads(body)


def receive_response(
    process: subprocess.Popen[bytes], expected_id: int, timeout: float = 10.0
) -> dict:
    """Read until the response for ``expected_id`` arrives.

    LSP servers are allowed to emit notifications (for example
    ``window/logMessage``) between a request and its response. Treating the
    first frame as the response makes a release smoke test flaky even though
    the server is protocol-correct.
    """

    deadline = time.monotonic() + timeout
    while True:
        remaining = max(0.1, deadline - time.monotonic())
        message = receive(process, remaining)
        if message.get("id") == expected_id:
            return message
        if "method" in message and "id" not in message:
            continue
        raise RuntimeError(f"unexpected LSP response while waiting for id {expected_id}: {message}")


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: lsp_protocol_smoke.py PATH", file=sys.stderr)
        return 2
    process = subprocess.Popen(
        [sys.argv[1]], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE
    )
    try:
        send(
            process,
            {
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "processId": None,
                    "rootUri": None,
                    "capabilities": {"general": {"positionEncodings": ["utf-16"]}},
                },
            },
        )
        response = receive_response(process, expected_id=1)
        if response.get("id") != 1 or "result" not in response:
            raise RuntimeError(f"invalid initialize response: {response}")
        send(process, {"jsonrpc": "2.0", "method": "initialized", "params": {}})
        send(process, {"jsonrpc": "2.0", "id": 2, "method": "shutdown"})
        response = receive_response(process, expected_id=2)
        if response.get("id") != 2 or "result" not in response:
            raise RuntimeError(f"invalid shutdown response: {response}")
        send(process, {"jsonrpc": "2.0", "method": "exit"})
        # Some stdio transports keep their read loop alive until EOF even
        # after the LSP `exit` notification. Closing stdin makes termination
        # deterministic for packaged smoke runs without changing the protocol
        # sequence observed by the server.
        process.stdin.close()
        process.wait(timeout=10)
        return 0 if process.returncode == 0 else process.returncode
    finally:
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                process.kill()


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:  # noqa: BLE001 - concise smoke failure
        print(f"LSP protocol smoke failed: {error}", file=sys.stderr)
        raise SystemExit(1)
