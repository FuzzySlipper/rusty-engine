#!/usr/bin/env python3
"""Locate one live supervised CoreCLR worker through Linux procfs."""

import argparse
import json
import os
from pathlib import Path
import stat
from typing import Iterable, Optional


MAX_CMDLINE_BYTES = 64 * 1024
MAX_ENV_BYTES = 1024 * 1024
MAX_MAPS_BYTES = 4 * 1024 * 1024


def fail(message: str) -> "None":
    raise SystemExit(f"find-coreclr-worker: {message}")


def parse_arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Locate one live supervised CoreCLR worker through /proc.",
    )
    parser.add_argument(
        "--project",
        type=Path,
        help="C# project file or directory containing its staged products",
    )
    parser.add_argument(
        "--product",
        type=Path,
        help="exact staged Product directory",
    )
    parser.add_argument("--pid", type=positive_pid, help="inspect only this process")
    options = parser.parse_args()
    if options.project is None and options.product is None:
        parser.error("one of --project or --product is required")
    return options


def positive_pid(value: str) -> int:
    try:
        pid = int(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("must be a positive process id") from error
    if pid <= 0:
        raise argparse.ArgumentTypeError("must be a positive process id")
    return pid


def absolute_path(path: Path) -> Path:
    return path.expanduser().resolve(strict=False)


def project_directory(path: Path) -> Path:
    path = absolute_path(path)
    return path.parent if path.suffix.lower() == ".csproj" else path


def proc_pids(selected_pid: Optional[int]) -> Iterable[int]:
    if selected_pid is not None:
        yield selected_pid
        return
    try:
        entries = os.scandir("/proc")
    except OSError as error:
        fail(f"cannot enumerate /proc: {error.strerror or error}")
    with entries:
        for entry in entries:
            if entry.name.isdecimal():
                yield int(entry.name)


def read_proc_bytes(path: Path, maximum: int) -> Optional[bytes]:
    try:
        with path.open("rb") as source:
            data = source.read(maximum + 1)
    except (FileNotFoundError, PermissionError, ProcessLookupError, OSError):
        return None
    return data if len(data) <= maximum else None


def command_line(pid: int) -> Optional[list[str]]:
    raw = read_proc_bytes(Path("/proc") / str(pid) / "cmdline", MAX_CMDLINE_BYTES)
    if not raw:
        return None
    return [os.fsdecode(value) for value in raw.split(b"\0") if value]


def option_value(arguments: list[str], option: str) -> Optional[str]:
    values: list[str] = []
    for index, value in enumerate(arguments[:-1]):
        if value == option:
            values.append(arguments[index + 1])
    return values[0] if len(values) == 1 else None


def is_coreclr_worker(pid: int, arguments: list[str]) -> Optional[Path]:
    product = option_value(arguments, "--product")
    worker_channel = option_value(arguments, "--worker-channel")
    if (
        product is None
        or worker_channel is None
        or "--loader" not in arguments
        or option_value(arguments, "--loader") != "coreclr"
        or "--worker" not in arguments
    ):
        return None
    path = Path(product)
    if not path.is_absolute():
        try:
            path = (Path("/proc") / str(pid) / "cwd").resolve(strict=True) / path
        except OSError:
            return None
    return absolute_path(path)


def coreclr_loaded(pid: int) -> bool:
    raw = read_proc_bytes(Path("/proc") / str(pid) / "maps", MAX_MAPS_BYTES)
    if raw is None:
        return False
    return any(b"libcoreclr.so" in line for line in raw.splitlines())


def process_stat(pid: int) -> Optional[tuple[int, str]]:
    raw = read_proc_bytes(Path("/proc") / str(pid) / "stat", 16 * 1024)
    if raw is None:
        return None
    closing_parenthesis = raw.rfind(b")")
    if closing_parenthesis < 0:
        return None
    fields = raw[closing_parenthesis + 2 :].split()
    # Fields after `comm` start at /proc stat field 3; starttime is field 22.
    if len(fields) <= 19 or not fields[1].isdigit() or not fields[19].isdigit():
        return None
    return int(fields[1]), fields[19].decode("ascii")


def temporary_directory(pid: int) -> Optional[Path]:
    raw = read_proc_bytes(Path("/proc") / str(pid) / "environ", MAX_ENV_BYTES)
    if raw is None:
        return None
    for entry in raw.split(b"\0"):
        if entry.startswith(b"TMPDIR="):
            value = entry[len(b"TMPDIR=") :]
            if value:
                path = Path(os.fsdecode(value))
                if not path.is_absolute():
                    try:
                        path = (Path("/proc") / str(pid) / "cwd").resolve(strict=True) / path
                    except OSError:
                        return None
                return path
            return Path("/tmp")
    return Path("/tmp")


def diagnostic_port(pid: int, starttime: str) -> Optional[Path]:
    tmpdir = temporary_directory(pid)
    if tmpdir is None:
        return None
    endpoint = tmpdir / f"dotnet-diagnostic-{pid}-{starttime}-socket"
    try:
        endpoint_stat = endpoint.stat()
    except (FileNotFoundError, PermissionError, OSError):
        return None
    return endpoint if stat.S_ISSOCK(endpoint_stat.st_mode) else None


def under_directory(path: Path, directory: Path) -> bool:
    try:
        path.relative_to(directory)
    except ValueError:
        return False
    return True


def worker_record(
    pid: int,
    expected_product: Optional[Path],
    expected_project: Optional[Path],
) -> Optional[dict[str, object]]:
    arguments = command_line(pid)
    if arguments is None:
        return None
    product = is_coreclr_worker(pid, arguments)
    if product is None:
        return None
    if expected_product is not None and product != expected_product:
        return None
    if expected_project is not None and not under_directory(product, expected_project):
        return None
    identity = process_stat(pid)
    if identity is None:
        return None
    parent, starttime = identity
    if not coreclr_loaded(pid):
        return None
    port = diagnostic_port(pid, starttime)
    if port is None:
        return None
    # Do not report a record assembled across a process exit or PID reuse.
    if process_stat(pid) != identity:
        return None
    return {
        "pid": pid,
        "parentPid": parent,
        "runtimeInstanceId": option_value(arguments, "--runtime-instance-id"),
        "productDirectory": str(product),
        "coreclr": True,
        "diagnosticPort": str(port),
    }


def main() -> None:
    options = parse_arguments()
    expected_product = absolute_path(options.product) if options.product else None
    expected_project = project_directory(options.project) if options.project else None
    matches = [
        record
        for pid in proc_pids(options.pid)
        if (record := worker_record(pid, expected_product, expected_project)) is not None
    ]
    if not matches:
        if options.pid is not None:
            fail(f"pid {options.pid} is not a live matching CoreCLR worker with a diagnostic socket")
        fail("no live matching CoreCLR worker with a diagnostic socket")
    if len(matches) != 1:
        fail(f"multiple matching CoreCLR workers found: {', '.join(str(record['pid']) for record in matches)}; pass --pid")
    print(json.dumps(matches[0], separators=(",", ":")))


if __name__ == "__main__":
    main()
