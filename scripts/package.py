"""Development-only reproducible file selection for the two distribution ZIPs."""
from pathlib import Path
import hashlib
import json
import stat
import zipfile

root = Path(__file__).resolve().parent.parent
out = root / "dist"
out.mkdir(exist_ok=True)
files = [root / name for name in (
    ".gitignore", "Cargo.toml", "Cargo.lock", "build.rs", "README.md",
    "package.json", "package-lock.json",
)]
for directory in ("src", "tests", "examples", "scripts", "web", "docs"):
    files.extend(p for p in (root / directory).rglob("*")
                 if p.is_file() and not any(part == "__pycache__" for part in p.parts)
                 and p.suffix != ".log")
files.extend(root / "lean" / name for name in (
    "lean-toolchain", "lakefile.toml", "lake-manifest.json", "Recurrence.lean",
    "Recurrence/Theory.lean", "Audit.lean",
))
manifest = {"version": "0.2.0", "artifacts": []}
for binary in (False, True):
    name = "recurrence-lab-linux-x86_64.zip" if binary else "recurrence-lab-source.zip"
    selected = [(p, "recurrence-lab/" + p.relative_to(root).as_posix()) for p in sorted(set(files))]
    if binary:
        selected.append((root / "target/release/recurrence-lab", "recurrence-lab/recurrence-lab"))
    target = out / name
    with zipfile.ZipFile(target, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for path, arcname in selected:
            info = zipfile.ZipInfo(arcname, (2026, 10, 9, 0, 0, 0))
            info.create_system = 3
            info.external_attr = (stat.S_IFREG | (path.stat().st_mode & 0o777)) << 16
            info.compress_type = zipfile.ZIP_DEFLATED
            archive.writestr(info, path.read_bytes())
    with zipfile.ZipFile(target) as archive:
        assert archive.testzip() is None
        manifest["artifacts"].append({"file": name, "bytes": target.stat().st_size,
            "entries": len(archive.infolist()),
            "uncompressed_bytes": sum(entry.file_size for entry in archive.infolist()),
            "sha256": hashlib.sha256(target.read_bytes()).hexdigest()})
(out / "artifact-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
print(json.dumps(manifest, indent=2))
