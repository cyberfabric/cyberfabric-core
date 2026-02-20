#!/usr/bin/env python3
"""
Generate OpenAPI spec by starting hyperspot-server, fetching the spec, and stopping the server.
Cross-platform compatible (Windows/Linux/macOS).
"""

import argparse
import os
import signal
import subprocess
import sys
import time
import urllib.request
import urllib.error
from pathlib import Path
from urllib.parse import urlparse

def _validate_http_url(url: str) -> None:
    scheme = urlparse(url).scheme
    if scheme not in ("http", "https"):
        raise ValueError(f"Unsupported URL scheme: {scheme}")

def wait_for_server(url: str, max_wait: int = 300) -> bool:
    """Wait for the server to become ready."""
    _validate_http_url(url)
    start = time.monotonic()
    elapsed = 0
    sleep_time = 1
    while elapsed < max_wait:
        try:
            with urllib.request.urlopen(url, timeout=5) as resp:
                if resp.status == 200:
                    return True
        except (urllib.error.URLError, urllib.error.HTTPError, TimeoutError):
            pass
        print(f"Waiting for server... ({int(elapsed)}s)")
        time.sleep(sleep_time)
        elapsed = time.monotonic() - start
        sleep_time = min(sleep_time * 2, 8)
    return False


def fetch_openapi(url: str, output_path: Path) -> bool:
    """Fetch OpenAPI spec and save to file."""
    _validate_http_url(url)
    try:
        with urllib.request.urlopen(url, timeout=30) as resp:
            content = resp.read()
            output_path.parent.mkdir(parents=True, exist_ok=True)
            output_path.write_bytes(content)
            return True
    except (urllib.error.URLError, urllib.error.HTTPError) as e:
        print(f"ERROR: Failed to fetch OpenAPI spec: {e}")
        return False


def main():
    parser = argparse.ArgumentParser(description="Generate OpenAPI spec from hyperspot-server")
    parser.add_argument("--url", default="http://127.0.0.1:8087/openapi.json",
                        help="OpenAPI endpoint URL")
    parser.add_argument("--output", default="docs/api/api.json",
                        help="Output file path")
    parser.add_argument("--config", required=True,
                        help="Server config file path")
    parser.add_argument("--server-bin", default="target/debug/hyperspot-server",
                        help="Path to hyperspot-server binary")
    parser.add_argument("--max-wait", type=int, default=300,
                        help="Maximum seconds to wait for server")
    args = parser.parse_args()

    if not Path(args.config).exists():
        print(f"ERROR: Server config not found: {args.config}")
        sys.exit(1)

    # Determine binary path (add .exe on Windows)
    server_bin = args.server_bin
    if sys.platform == "win32" and not server_bin.endswith(".exe"):
        server_bin += ".exe"

    if not Path(server_bin).exists():
        print(f"ERROR: Server binary not found: {server_bin}")
        print("Run 'cargo build --bin hyperspot-server --features users-info-example' first")
        sys.exit(1)

    print(f"Starting hyperspot-server with config: {args.config}")
    
    # Start server as subprocess
    try:
        # Use CREATE_NEW_PROCESS_GROUP on Windows for proper signal handling
        kwargs = {}
        if sys.platform == "win32":
            kwargs["creationflags"] = subprocess.CREATE_NEW_PROCESS_GROUP
        else:
            kwargs["start_new_session"] = True

        server_proc = subprocess.Popen(
            [server_bin, "--config", args.config],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            **kwargs
        )
    except OSError as e:
        print(f"ERROR: Failed to start server: {e}")
        sys.exit(1)

    print(f"hyperspot-server PID: {server_proc.pid}")

    try:
        print(f"Waiting for {args.url} to become ready...")
        if not wait_for_server(args.url, args.max_wait):
            print("ERROR: hyperspot-server did not become ready in time")
            sys.exit(1)

        print("Server is ready, fetching OpenAPI spec...")
        output_path = Path(args.output)
        if not fetch_openapi(args.url, output_path):
            sys.exit(1)

        print(f"OpenAPI spec saved to {args.output}")

    finally:
        print("Stopping hyperspot-server...")
        try:
            if sys.platform == "win32":
                # On Windows, terminate() sends SIGTERM equivalent
                server_proc.terminate()
            else:
                # On Unix, send SIGTERM to process group
                os.killpg(os.getpgid(server_proc.pid), signal.SIGTERM)
        except (ProcessLookupError, OSError):
            pass  # Process already terminated

        # Wait for process to finish
        try:
            server_proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            print("Server did not stop gracefully, forcing kill...")
            server_proc.kill()
            server_proc.wait()

    print("Done.")


if __name__ == "__main__":
    main()
