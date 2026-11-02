#!/usr/bin/env python3
"""Build release/<VERSION>/<linux|windows|macOs> without publishing anything."""
import argparse
import platform
import plistlib
from pathlib import Path
import re
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def run(*args, cwd=ROOT):
    subprocess.run([str(arg) for arg in args], cwd=cwd, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    selection = parser.add_mutually_exclusive_group()
    selection.add_argument("--platform", choices=("linux", "windows", "macOs"))
    selection.add_argument("--all", action="store_true", help="On macOS, also cross-build Linux amd64 and Windows portable artifacts.")
    parser.add_argument("--linux-target", help="Cross-build a Linux target with cargo-zigbuild, e.g. x86_64-unknown-linux-gnu.2.31.")
    args = parser.parse_args()
    version = (ROOT / "VERSION").read_text().strip()
    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        parser.error("VERSION must contain a numeric major.minor.patch version")
    for file, pattern in (
        (ROOT / "apps/linux/Cargo.toml", r'^version = "([^"]+)"'),
        (ROOT / "apps/windows/Moonglow.csproj", r'<Version>([^<]+)</Version>'),
    ):
        if re.search(pattern, file.read_text(), re.M).group(1) != version:
            parser.error(f"{file.relative_to(ROOT)} must match VERSION ({version})")
    with (ROOT / "apps/macos/Packaging/Info.plist").open("rb") as plist:
        if plistlib.load(plist)["CFBundleShortVersionString"] != version:
            parser.error("macOS Info.plist must match VERSION")
    host = {"Darwin": "macOs", "Windows": "windows", "Linux": "linux"}.get(platform.system())
    if host is None:
        parser.error("unsupported build host")
    if args.all and host != "macOs":
        parser.error("--all requires a Mac with the cross-build prerequisites; use --platform on native runners")
    selected = ["linux", "windows", "macOs"] if args.all else [args.platform or host]
    release = ROOT / "release" / version
    for name in ("linux", "windows", "macOs"):
        (release / name).mkdir(parents=True, exist_ok=True)
    for name in selected:
        if name == "macOs":
            if host != "macOs":
                parser.error("macOS artifacts require Xcode on a Mac")
            run("bash", ROOT / "apps/macos/Packaging/build-app.sh")
        elif name == "windows":
            powershell = shutil.which("pwsh") or shutil.which("powershell")
            if not powershell:
                parser.error("Windows packaging requires PowerShell and the .NET SDK")
            flags = [] if host == "windows" else ["-SkipInstaller"]
            run(powershell, "-NoProfile", "-File", ROOT / "apps/windows/build.ps1", *flags)
        else:
            target = args.linux_target or ("x86_64-unknown-linux-gnu.2.31" if host != "linux" else None)
            app = ROOT / "apps/linux"
            if target:
                if not shutil.which("cargo-zigbuild") or not shutil.which("zig"):
                    parser.error("Linux cross-builds require zig, cargo-zigbuild, and the Rust target (see README)")
                run("cargo", "zigbuild", "--release", "--locked", "-p", "moonglow", "--target", target, cwd=app)
                binary = app / "target" / target.split(".")[0] / "release/moonglow"
                arch = "arm64" if target.startswith("aarch64") else "amd64"
            else:
                run("cargo", "build", "--release", "--locked", "-p", "moonglow", cwd=app)
                binary = app / "target/release/moonglow"
                arch = "arm64" if platform.machine().lower() in ("aarch64", "arm64") else "amd64"
            run(sys.executable, ROOT / "tools/package-linux.py", "--binary", binary, "--arch", arch)
    print(f"Release artifacts: {release}")


if __name__ == "__main__":
    main()
