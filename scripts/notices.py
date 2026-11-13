"""Collect original license files from the locked Cargo source packages."""
import json
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parents[1]
metadata = json.loads(subprocess.check_output(
    ["cargo", "metadata", "--locked", "--format-version", "1"], cwd=root, text=True
))
texts = [(root / "NOTICE").read_text(), (root / "licenses/TUIOS-MIT.txt").read_text()]
for package in sorted(metadata["packages"], key=lambda item: (item["name"], item["version"])):
    if package["source"] is None:
        continue
    directory = Path(package["manifest_path"]).parent
    files = sorted(path for path in directory.iterdir()
                   if path.is_file() and path.name.upper().startswith(("LICENSE", "COPYING", "NOTICE")))
    if package.get("license_file"):
        license_file = directory / package["license_file"]
        if license_file not in files:
            files.append(license_file)
    if not files:
        raise SystemExit(f"No original license files found for {package['name']} {package['version']}")
    texts.append(f"\n--- {package['name']} {package['version']} ({package['license']}) ---\n")
    for path in files:
        text = path.read_text()
        if not text.strip():
            raise SystemExit(f"Empty license: {path}")
        texts.append(f"\n{path.name}\n{text}\n")
Path(sys.argv[1]).write_text("\n".join(texts))
