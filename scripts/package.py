"""Package the paired component and UI Service without rebuilding either."""
from pathlib import Path
import sys
from zipfile import ZipFile, ZIP_DEFLATED

files = {
    "plugin.layout2.wasm": Path("build.nosync/plugin.layout2.wasm"),
    "ui-plugins/layout2.js": Path("src/ui-plugins/layout2.js"),
    "LICENSE": Path("LICENSE"),
    "NOTICE": Path("build.nosync/NOTICE"),
    "README.md": Path("README.md"),
}
with ZipFile(sys.argv[1], "w", ZIP_DEFLATED) as archive:
    for name, path in sorted(files.items()):
        archive.write(path, name)
