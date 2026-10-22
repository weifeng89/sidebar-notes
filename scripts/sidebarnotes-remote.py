#!/usr/bin/env python3
"""Run Sidebar Notes operations on the one Mac library over SSH."""

import argparse
import json
import os
import pathlib
import shlex
import subprocess
import sys


MAX_NOTE_BYTES = 8 * 1024 * 1024
MAX_ARTIFACT_BYTES = 512 * 1024 * 1024
DEFAULT_APP = "/Applications/Sidebar Notes.app/Contents/MacOS/stik"


def request(command):
    op = command[0] if command else ""
    if op == "import" and 3 <= len(command) <= 5:
        source = pathlib.Path(command[1])
        if not source.is_file() or source.is_symlink():
            raise ValueError("Import source must be a regular local file")
        size = source.stat().st_size
        if size > MAX_ARTIFACT_BYTES:
            raise ValueError("Artifact exceeds 512 MiB")
        return ["import", source.name, *command[2:]], size, source
    if op == "save" and len(command) == 3:
        return command, None, sys.stdin.buffer
    if (op == "list" and len(command) in (1, 2)) or (
        op in ("search", "show", "mkdir", "create") and len(command) == 2
    ) or (op == "folders" and len(command) == 1):
        return command, 0, None
    raise ValueError("Invalid Sidebar Notes remote command")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default=os.environ.get("SIDEBAR_NOTES_SSH_TARGET", "sidebarnotes-mac"))
    parser.add_argument("--app", default=os.environ.get("SIDEBAR_NOTES_MAC_APP", DEFAULT_APP))
    parser.add_argument("--ssh", default="ssh", help=argparse.SUPPRESS)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    options = parser.parse_args(argv)
    if not options.host:
        parser.error("Set --host or SIDEBAR_NOTES_SSH_TARGET")
    if options.host.startswith("-") or any(c.isspace() for c in options.host):
        parser.error("SSH host must be one host or alias")
    try:
        command, size, source = request(options.command)
        if size is None:
            payload = source.read(MAX_NOTE_BYTES + 1)
            size = len(payload)
            if size > MAX_NOTE_BYTES:
                raise ValueError("Note exceeds 8 MiB")
            source = None
        else:
            payload = b""
        header = json.dumps(
            {"version": 1, "command": command, "payload_bytes": size},
            separators=(",", ":"),
        ).encode("utf-8") + b"\n"
        remote_command = shlex.quote(options.app) + " remote"
        process = subprocess.Popen(
            [options.ssh, "-T", "-o", "BatchMode=yes", "-o", "ConnectTimeout=10", options.host, remote_command],
            stdin=subprocess.PIPE,
        )
        try:
            process.stdin.write(header)
            if source is not None:
                with source.open("rb") as artifact:
                    while chunk := artifact.read(1024 * 1024):
                        process.stdin.write(chunk)
            else:
                process.stdin.write(payload)
        except BrokenPipeError:
            pass  # The Mac-side process reports the protocol error on stderr.
        except OSError:
            process.kill()
            process.wait()
            raise
        finally:
            if not process.stdin.closed:
                try:
                    process.stdin.close()
                except BrokenPipeError:
                    pass
        return process.wait()
    except (OSError, ValueError) as error:
        print(json.dumps({"error_code": "client_failed", "message": str(error)}), file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
