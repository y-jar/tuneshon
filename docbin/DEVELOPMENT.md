# Development

Guide for building, testing, and packaging tuneshon locally.

## Tools

- `nix` with flakes enabled, or plain `cargo`.
- The flake pulls `nixos-unstable` and `rust-overlay` for the Rust toolchain.

## Development shell

```sh
nix develop
```

This provides `cargo`, `rustc`, `rust-src`, `rustfmt`, `clippy`, `pkg-config`,
and the GTK/Wayland/X11 libraries that `eframe` (egui) needs. It also defines
helpers:

- `run`: `cargo run -- "$@"`
- `build`: `cargo build --release`
- `test`: `cargo test "$@"`

A classic `shell.nix` provides the same helpers for non-flake setups.

## Commands

```sh
nix develop --command cargo check
nix develop --command cargo test
nix develop --command cargo build --release
nix develop --command cargo run -- --gui
```

Run a specific test:

```sh
nix develop --command cargo test gui::actions
```

## Layout

See [docbin/ARCHITECTURE.md](ARCHITECTURE.md) for the module map and data flow.

## Flake packaging

The package is built with `pkgs.rustPlatform.buildRustPackage`. Key points:

- The source is filtered with `cleanSourceWith` so only needed files are
  copied (`Cargo.toml`, `Cargo.lock`, `flake.*`, `src/`, `tuneshon_logo.png`).
- `cleanSourceWith` follows git. New untracked files (for example a new
  `src/*.rs`) are excluded from the build until they are `git add`ed. Stage new
  source files before building the package.
- The binary is wrapped with `wrapProgram`, baking `TUNESHON_CONFIG_DIR` and
  the GTK `LD_LIBRARY_PATH`.
- A `.desktop` entry and the launcher icon are installed to `share/` so the app
  shows up in desktop launchers as "update".
- `lib.mkPackage { pkgs, configDir }` lets consumers bake a specific default
  config dir.

Build a package with:

```sh
nix build .#tuneshon
```

## Tests

Tests cover command-string construction (`gui::actions::tests`), generation
parsing (`generations::tests`), terminal color classification
(`gui::terminal::tests`), and `flake.lock` parsing (`flakelock::tests`). Run
them with `cargo test`.

## Conventions

- Comments are concise and specific; avoid editorializing and unnecessary
  punctuation like em dashes.
- Commands built by `actions.rs` are covered by unit tests that assert the
  exact string, so keep the expected-form assertions in sync with any change.
- Name functions and variables to say what they mean rather than how they are
  implemented.