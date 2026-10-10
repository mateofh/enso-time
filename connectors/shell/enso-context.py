#!/usr/bin/env python3
"""Minimal Ensō Time shell connector. It never reads commands, history, env values or file content."""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
import uuid
from typing import Optional

PORTS = range(5610, 5700)


def config_path() -> pathlib.Path:
    if sys.platform == "darwin":
        root = pathlib.Path.home() / "Library" / "Application Support" / "Ensō Time" / "connectors"
    elif os.name == "nt":
        root = pathlib.Path(os.environ.get("LOCALAPPDATA", pathlib.Path.home())) / "Enso Time" / "connectors"
    else:
        root = pathlib.Path(os.environ.get("XDG_CONFIG_HOME", pathlib.Path.home() / ".config")) / "enso-time"
    root.mkdir(parents=True, exist_ok=True, mode=0o700)
    return root / "shell.json"


def load_config() -> dict:
    path = config_path()
    if not path.exists():
        return {}
    return json.loads(path.read_text(encoding="utf-8"))


def save_config(value: dict) -> None:
    path = config_path()
    handle, temporary = tempfile.mkstemp(prefix="shell-", suffix=".json", dir=path.parent)
    try:
        with os.fdopen(handle, "w", encoding="utf-8") as stream:
            json.dump(value, stream)
        os.chmod(temporary, 0o600)
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def request(port: int, route: str, payload: Optional[dict] = None, token: Optional[str] = None) -> dict:
    headers = {"Content-Type": "application/json"}
    if token:
        headers["Authorization"] = f"Bearer {token}"
    data = json.dumps(payload).encode() if payload is not None else None
    req = urllib.request.Request(f"http://127.0.0.1:{port}{route}", data=data, headers=headers, method="POST" if data else "GET")
    with urllib.request.urlopen(req, timeout=0.8) as response:
        return json.loads(response.read() or b"{}")


def find_port(preferred: Optional[int] = None) -> int:
    for port in ([preferred] if preferred else PORTS):
        if not port:
            continue
        try:
            request(port, "/api/0/info")
            return port
        except (OSError, urllib.error.URLError, json.JSONDecodeError):
            continue
    raise RuntimeError("No se encontró Ensō Time entre los puertos 5610–5699")


def pair(code: str, preferred: Optional[int]) -> None:
    if len(code) != 8 or not code.isdigit():
        raise RuntimeError("El código debe tener ocho dígitos")
    current = load_config()
    port = find_port(preferred or current.get("port"))
    client_id = current.get("clientId") or str(uuid.uuid4())
    result = request(port, "/connectors/v1/pair", {"code": code, "clientId": client_id})
    save_config({"port": port, "clientId": result["clientId"], "token": result["token"], "lastSentAt": 0, "lastCwd": None})
    print(f"Ensō Time emparejado en 127.0.0.1:{port}")


def git_value(cwd: str, *args: str) -> str | None:
    try:
        result = subprocess.run(["git", "-C", cwd, *args], capture_output=True, text=True, timeout=0.5, check=True)
        return result.stdout.strip() or None
    except (OSError, subprocess.SubprocessError):
        return None


def send() -> None:
    current = load_config()
    if not current.get("token") or not current.get("port"):
        return
    cwd = str(pathlib.Path.cwd())
    now_epoch = time.time()
    if current.get("lastCwd") == cwd and now_epoch - float(current.get("lastSentAt", 0)) < 60:
        return
    now = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    repository = git_value(cwd, "rev-parse", "--show-toplevel")
    branch = git_value(cwd, "branch", "--show-current")
    payload = {
        "schemaVersion": 2,
        "id": f"{current['clientId']}-{int(now_epoch * 1000)}",
        "source": "shell",
        "observedAt": now,
        "durationSeconds": 0,
        "app": "Terminal",
        "title": None,
        "domain": None,
        "project": pathlib.Path(cwd).name,
        "repository": pathlib.Path(repository).name if repository else None,
        "branch": branch,
        "fileType": None,
        "textExcerpt": None,
        "ocrTerms": [],
        "sensitivity": "normal",
        "clientId": current["clientId"],
        "createdAt": now,
    }
    try:
        request(current["port"], "/connectors/v1/observations", payload, current["token"])
    except urllib.error.HTTPError as error:
        if error.code == 401:
            current.pop("token", None)
            save_config(current)
        return
    except (OSError, urllib.error.URLError):
        return
    current.update({"lastSentAt": now_epoch, "lastCwd": cwd})
    save_config(current)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--pair", metavar="CODE")
    parser.add_argument("--port", type=int)
    parser.add_argument("--send", action="store_true")
    args = parser.parse_args()
    if args.pair:
        pair(args.pair, args.port)
    elif args.send:
        send()
    else:
        parser.print_help()


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"Ensō Time: {error}", file=sys.stderr)
        raise SystemExit(1)
