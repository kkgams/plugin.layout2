"""Embed distribution notices in a freshly stripped WASM component."""
from pathlib import Path
import sys


def leb(value):
    result = bytearray()
    while True:
        byte = value & 127
        value >>= 7
        result.append(byte | (128 if value else 0))
        if not value:
            return bytes(result)


def section(name, text):
    name = name.encode()
    data = leb(len(name)) + name + text
    return b"\x00" + leb(len(data)) + data


source, output = map(Path, sys.argv[1:])
if source.resolve() == output.resolve():
    raise SystemExit("Input and output must differ")
component = source.read_bytes()
if component[:8] != b"\x00asm\x0d\x00\x01\x00":
    raise SystemExit("Expected a WebAssembly component")
for name, path in (("gams.license", Path("LICENSE")), ("gams.notice", Path("build.nosync/NOTICE"))):
    text = path.read_bytes()
    if not text.strip():
        raise SystemExit(f"Empty notice: {path}")
    component += section(name, text)
output.write_bytes(component)
