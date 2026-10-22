# Sidebar Notes

Sidebar Notes is a local macOS library for prompts, notes, and project artifacts. The app starts as three edge bookmarks: **Prompt**, **Notes**, and **Artifacts**. Clicking one opens the 320 px sidebar. The Artifacts bookmark maps to `Docs/` on disk.

The library lives at `~/Documents/SideNotes/` and uses ordinary folders and files. Notes and descriptions are standard Markdown; delivered files stay in their original formats. Finder changes are watched, deletions go to macOS Trash, and local logs omit document contents and absolute paths. The CLI operates on this same library so local and remote agents share one source of truth; see [the agent interface guide](docs/agent-interface.md).

The project is a fork of [Stik](https://github.com/0xMassi/stik_app). Existing capture/editor features, their data, and the upstream MIT license are retained. Stik-specific historical material is kept in the source tree for reference; it is not current Sidebar Notes product or release documentation.

## Install or build

This repository builds an ad-hoc signed Apple Silicon `.app` for local installation. It is not Developer ID signed or notarized. On first open, macOS may require opening it from Finder's context menu.

Requirements: macOS 14+, Apple Silicon, Xcode 26+/Swift 6.2+, Rust stable, Node 20+, Bun 1.4.1, and `protoc`. Initialize the DarwinKit submodule and install the locked JavaScript dependencies:

```bash
git submodule update --init --recursive
bun install --frozen-lockfile
```

Build the release app from the repository root:

```bash
swift build --package-path src-tauri/darwinkit --configuration release --arch arm64
install -m 755 \
  "$(swift build --package-path src-tauri/darwinkit --configuration release --arch arm64 --show-bin-path)/darwinkit" \
  src-tauri/binaries/darwinkit-aarch64-apple-darwin
bun run tauri build --target aarch64-apple-darwin --bundles app \
  --config '{"bundle":{"createUpdaterArtifacts":false}}'
```

Tauri's Rust strip helper may fail on some Xcode toolchains because its bundled `rust-objcopy` cannot load `libLLVM.dylib`. The build still completes; strip the two app executables with Apple's tool and re-sign the bundle:

```bash
APP="src-tauri/target/aarch64-apple-darwin/release/bundle/macos/Sidebar Notes.app"
/usr/bin/strip -x -S "$APP/Contents/MacOS/stik" "$APP/Contents/MacOS/darwinkit"
codesign --force --deep --sign - "$APP"
```

The resulting bundle was 13,872 KiB on the tested Apple Silicon Mac. Verify the bundle before installing it:

```bash
codesign --verify --deep --strict --verbose=2 "$APP"
open "$APP"
```

To install locally, copy that `.app` into `/Applications` using Finder or `ditto`. Release app data is not isolated: it uses the normal user profile and `~/Documents/SideNotes/`. Use `./scripts/build-dev.sh qa` for isolated UI testing instead. Run `./scripts/verify.sh` for repository gates.

## Data and compatibility

- `Prompt/`, `Notes/`, and `Docs/` are real directories under the selected library root.
- The app's CLI executable and Rust library retain their internal names (`stik` and `stik_lib`) for compatibility with the existing agent command and the inherited codebase. The Cargo package and user-facing product are named Sidebar Notes.
- The original Stik library at `~/Documents/Stik/`, settings, and compatibility features remain separate from the Sidebar Notes library.
- MIT terms and the upstream attribution are in [LICENSE](LICENSE) and [NOTICE](NOTICE).

## Historical upstream documentation

The upstream Stik README, security policy, and Homebrew template are archived in [docs/upstream/](docs/upstream/). The retained roadmap, mobile plan, changelog, and release checklist are labeled as historical. [CONTRIBUTING.md](CONTRIBUTING.md) and [SECURITY.md](SECURITY.md) describe this fork.
