"""Create an allowlisted source archive from the current working tree, not Git HEAD."""
from pathlib import Path
import hashlib
import io
import json
import tarfile
import argparse

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--output-dir", default="output/sprint20")
args = parser.parse_args()
OUT = ROOT / args.output_dir
OUT.mkdir(parents=True, exist_ok=True)
files = [ROOT / "Cargo.toml", ROOT / "Cargo.lock"]
for directory in ("src", "tests", "examples"):
    files.extend(sorted((ROOT / directory).rglob("*.rs")))
files.extend(sorted((ROOT / "assets/branding").glob("*.png")))
files.extend(sorted((ROOT / "assets/network").glob("*.der")))
# Deliberately public artificial test keys, never deployment keys.
files.extend(p for p in sorted((ROOT / "tests/fixtures/tls").glob("*")) if p.is_file())
for directory in ("deploy/linux",):
    if (ROOT / directory).exists():
        files.extend(p for p in sorted((ROOT / directory).rglob("*")) if p.is_file())
manifest = {p.relative_to(ROOT).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in files}
identity = hashlib.sha256(json.dumps(manifest, sort_keys=True).encode()).hexdigest()
data = json.dumps({"source_id": identity, "files": manifest}, indent=2).encode()
(OUT / "source-manifest.json").write_bytes(data)
archive = OUT / "linux-source.tar.gz"
with tarfile.open(archive, "w:gz") as tar:
    for p in files:
        tar.add(p, arcname=p.relative_to(ROOT).as_posix(), recursive=False)
    info = tarfile.TarInfo("source-manifest.json")
    info.size = len(data)
    tar.addfile(info, io.BytesIO(data))
print(f"Source {identity}; {len(files)} files; {archive.stat().st_size} bytes")
