SHELL := bash
.ONESHELL:
.SHELLFLAGS := -eu -o pipefail -c
.DELETE_ON_ERROR:
.DEFAULT_GOAL := build

PYTHON ?= python3
CARGO ?= cargo
CURL ?= curl
WASM_TOOLS ?= wasm-tools
DOCS_PORT ?= 8082
WIKI_VERSION := v1.8.0
WIKI_REPOSITORY := https://github.com/justgook/wiki
WIKI_MAKEFILE := build.nosync/wiki-$(WIKI_VERSION).mk
WIKI_ENGINE := $(abspath build.nosync/wiki-$(WIKI_VERSION))
GAMS_VERSION := v3.2.0
GAMS_ASSET := GAMS-macos-aarch64.zip
GAMS_RELEASE := https://github.com/kkgams/gams/releases/download/$(GAMS_VERSION)

export CARGO_HOME := $(abspath build.nosync/cargo-home)
export CARGO_TARGET_DIR := $(abspath build.nosync/cargo-target)
export TMPDIR := $(abspath build.nosync/tmp)
export XDG_CACHE_HOME := $(abspath build.nosync/cache)
export PYTHONPYCACHEPREFIX := $(abspath build.nosync/pycache)

RUST_SOURCES := $(wildcard src/*.rs)
WIT_SOURCES := $(wildcard wit/*.wit)

.PHONY: build test package open clean docs serve-docs validate-docs
build: build.nosync/plugin.layout2.wasm
package: build.nosync/plugin.layout2.zip build.nosync/SHA256SUMS

build.nosync/tmp:
	mkdir -p "$@"

# Cargo itself tracks its Rust dependency graph. The file target keeps Make
# incremental as well; src/wit directory mtimes also cover removed inputs.
build.nosync/gams_layout2.wasm: $(RUST_SOURCES) $(WIT_SOURCES) Cargo.toml Cargo.lock src wit Makefile | build.nosync/tmp
	$(CARGO) build --locked --release --target wasm32-wasip2
	cp "$(CARGO_TARGET_DIR)/wasm32-wasip2/release/gams_layout2.wasm" "$@"

build.nosync/NOTICE: Cargo.lock Cargo.toml NOTICE LICENSE licenses/TUIOS-MIT.txt scripts/notices.py Makefile | build.nosync/tmp
	$(PYTHON) scripts/notices.py "$@"

build.nosync/plugin.layout2.wasm: build.nosync/gams_layout2.wasm build.nosync/NOTICE LICENSE scripts/embed-notices.py Makefile
	$(WASM_TOOLS) strip -a "$<" -o build.nosync/layout2.stripped.wasm
	$(PYTHON) scripts/embed-notices.py build.nosync/layout2.stripped.wasm "$@"
	$(WASM_TOOLS) validate "$@"

build.nosync/plugin.layout2.zip: build.nosync/plugin.layout2.wasm build.nosync/NOTICE src/ui-plugins/layout2.js LICENSE README.md licenses/TUIOS-MIT.txt Makefile
	rm -f "$@"
	cd build.nosync && zip -q -X plugin.layout2.zip plugin.layout2.wasm NOTICE
	cd ../src && zip -q -X ../build.nosync/plugin.layout2.zip ui-plugins/layout2.js
	cd .. && zip -q -X build.nosync/plugin.layout2.zip LICENSE README.md licenses/TUIOS-MIT.txt

build.nosync/SHA256SUMS: build.nosync/plugin.layout2.zip
	cd build.nosync && shasum -a 256 plugin.layout2.zip > SHA256SUMS

test: | build.nosync/tmp
	$(CARGO) test --locked
	node --check src/ui-plugins/layout2.js
	node --test test/*.test.mjs

# Never update an already downloaded Host or alter macOS quarantine settings.
tools/gams.app: | build.nosync/tmp
	test "$$(uname -s)" = Darwin && test "$$(uname -m)" = arm64 || { echo 'Host download supports macOS Apple Silicon' >&2; exit 1; }
	test ! -e tools/gams-release-notices || { echo 'tools/gams-release-notices already exists' >&2; exit 1; }
	mkdir -p tools
	stage=$$(mktemp -d "$(TMPDIR)/gams-download.XXXXXX")
	trap 'rm -rf "$$stage"' EXIT
	$(CURL) -fLsS "$(GAMS_RELEASE)/$(GAMS_ASSET)" -o "$$stage/$(GAMS_ASSET)"
	$(CURL) -fLsS "$(GAMS_RELEASE)/SHA256SUMS-macos-aarch64" -o "$$stage/SHA256SUMS"
	(cd "$$stage" && shasum -a 256 -c SHA256SUMS)
	unzip -q "$$stage/$(GAMS_ASSET)" -d "$$stage/unpacked"
	test -f "$$stage/unpacked/GAMS.app/Contents/Info.plist"
	test -x "$$stage/unpacked/GAMS.app/Contents/MacOS/gams"
	mv "$$stage/unpacked/GAMS.app" "$$stage/GAMS.app"
	mv "$$stage/unpacked" tools/gams-release-notices
	mv "$$stage/GAMS.app" "$@"

open: build tools/gams.app
	GAMS_APP_CWD="$(CURDIR)" GAMS_CONFIG="gams.json" GAMS_WASMTIME_CACHE_DIR="$(CURDIR)/build.nosync/host-cache" ./tools/gams.app/Contents/MacOS/gams

$(WIKI_MAKEFILE): | build.nosync/tmp
	mkdir -p "$(@D)"
	trap 'rm -f "$@.tmp"' EXIT
	$(CURL) -fLsS --retry 3 "$(WIKI_REPOSITORY)/raw/$(WIKI_VERSION)/Makefile" -o "$@.tmp"
	mv "$@.tmp" "$@"

docs serve-docs validate-docs: $(WIKI_MAKEFILE) | build.nosync/tmp
	$(MAKE) -f "$(WIKI_MAKEFILE)" "$(if $(filter docs,$@),build,$(patsubst %-docs,%,$@))" WIKI_REPOSITORY="$(WIKI_REPOSITORY)" WIKI_VERSION="$(WIKI_VERSION)" WIKI_ENGINE="$(WIKI_ENGINE)" WIKI_SOURCE="$(abspath docs)" WIKI_OUTPUT="$(abspath build.nosync/docs)" PORT="$(DOCS_PORT)"

clean:
	$(PYTHON) -B scripts/clean.py
