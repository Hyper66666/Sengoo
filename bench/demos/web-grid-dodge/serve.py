#!/usr/bin/env python3
from __future__ import annotations

import argparse
import functools
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Serve web-grid-dodge demo locally.")
    parser.add_argument("--host", default="127.0.0.1", help="bind host (default: 127.0.0.1)")
    parser.add_argument("--port", type=int, default=8088, help="bind port (default: 8088)")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    demo_dir = Path(__file__).resolve().parent

    handler = functools.partial(SimpleHTTPRequestHandler, directory=str(demo_dir))
    httpd = ThreadingHTTPServer((args.host, args.port), handler)
    print(f"Serving {demo_dir} at http://{args.host}:{args.port}/")
    print("Press Ctrl+C to stop.")
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        httpd.server_close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
