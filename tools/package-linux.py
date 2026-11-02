#!/usr/bin/env python3
"""Package a built Linux ELF binary as a portable archive and a Debian package."""
import argparse
import gzip
import hashlib
import io
import os
from pathlib import Path
import re
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def package(binary, arch):
    version = (ROOT / "VERSION").read_text().strip()
    with binary.open("rb") as source:
        header = source.read(20)
    machine = {"amd64": 62, "arm64": 183}[arch]
    if len(header) < 20 or header[:6] != b"\x7fELF\x02\x01" or int.from_bytes(header[18:20], "little") != machine:
        raise ValueError(f"{binary} must be a 64-bit Linux ELF binary for {arch}")
    release = ROOT / "release" / version
    for platform in ("linux", "windows", "macOs"):
        (release / platform).mkdir(parents=True, exist_ok=True)
    output = release / "linux"
    epoch = int(os.environ.get("SOURCE_DATE_EPOCH", "0"))
    app = ROOT / "apps/linux"
    files = {"usr/bin/moonglow": (binary.read_bytes(), 0o755)}
    for source, destination in (
        (app / "packaging/moonglow.desktop", "usr/share/applications/moonglow.desktop"),
        (app / "packaging/moonglow.metainfo.xml", "usr/share/metainfo/io.github.prompt-smithy.moonglow.metainfo.xml"),
        (app / "packaging/51-moonglow-i2c.rules", "usr/lib/udev/rules.d/51-moonglow-i2c.rules"),
        (ROOT / "LICENSE", "usr/share/doc/moonglow/copyright"),
    ):
        files[destination] = (source.read_bytes(), 0o644)
    for size in (16, 24, 32, 48, 64, 128, 256, 512):
        relative = f"hicolor/{size}x{size}/apps/moonglow.png"
        files[f"usr/share/icons/{relative}"] = ((ROOT / "resources/linux" / relative).read_bytes(), 0o644)
    files["usr/share/man/man1/moonglow.1.gz"] = (gzip.compress((app / "packaging/moonglow.1").read_bytes(), mtime=epoch), 0o644)

    def archive(path, entries, prefix="."):
        with path.open("wb") as raw, gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=epoch) as compressed:
            with tarfile.open(fileobj=compressed, mode="w", format=tarfile.GNU_FORMAT) as tar:
                for name, (data, mode) in sorted(entries.items()):
                    info = tarfile.TarInfo(f"{prefix}/{name}")
                    info.size, info.mode, info.mtime = len(data), mode, epoch
                    info.uname = info.gname = "root"
                    tar.addfile(info, io.BytesIO(data))

    portable = output / f"moonglow-{version}-linux-{arch}.tar.gz"
    archive(portable, files, f"moonglow-{version}")
    control = (app / "packaging/debian/control").read_text().replace("@VERSION@", version)
    control = re.sub(r"^Architecture:.*$", f"Architecture: {arch}", control, flags=re.M)
    controls = {"control": (control.encode(), 0o644)}
    for name in ("postinst", "postrm"):
        controls[name] = ((app / "packaging/debian" / name).read_bytes(), 0o755)
    deb = output / f"moonglow_{version}_{arch}.deb"
    with tempfile.TemporaryDirectory(prefix="moonglow-deb-") as staging:
        staging = Path(staging)
        (staging / "debian-binary").write_text("2.0\n")
        archive(staging / "control.tar.gz", controls)
        archive(staging / "data.tar.gz", files)
        # Emit the common ar format directly. Apple's ar/ranlib treats inputs
        # as Mach-O library members and can discard these non-object payloads.
        temporary = deb.with_suffix(".deb.tmp")
        with temporary.open("wb") as archive_file:
            archive_file.write(b"!<arch>\n")
            for name in ("debian-binary", "control.tar.gz", "data.tar.gz"):
                data = (staging / name).read_bytes()
                header = f"{name + '/':<16}{epoch:<12}{0:<6}{0:<6}{'100644':<8}{len(data):<10}`\n".encode("ascii")
                if len(header) != 60:
                    raise ValueError("Debian ar member exceeds the common header limits")
                archive_file.write(header)
                archive_file.write(data)
                if len(data) % 2:
                    archive_file.write(b"\n")
        temporary.replace(deb)
    checksums = "".join(f"{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.name}\n" for p in sorted(output.iterdir()) if p.is_file() and p.name != "SHA256SUMS")
    (output / "SHA256SUMS").write_text(checksums)
    print(f"Created {portable}\nCreated {deb}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--arch", choices=("amd64", "arm64"), required=True)
    args = parser.parse_args()
    package(args.binary.resolve(), args.arch)
