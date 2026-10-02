# tuneshon

A lightweight NixOS update tool with a Rust GUI and headless CLI. It runs
`nh os build` as a non-elevated preview, then (after you confirm) applies it
with a single elevated `pkexec` call, streaming the output to an in-app
terminal.

## What you can do

- Click an action to update your NixOS config. First it builds the config so
  you can watch the output and see whether everything is correct.
- After a successful build, confirm to apply it. The tool prompts once for
  your password (via polkit) and runs the switch/boot/test.
- Preview the available actions and pick which flake inputs to update.
- Update just your packages, addons, or kernel, or update the app itself.
- See your system generations (newest first, current one highlighted).
- Toggle verbose activation logs on the terminal.

See [docbin/USAGE.md](docbin/USAGE.md) for a full walkthrough of the GUI and
CLI.

## Building and running

Requires Rust with the libraries for `eframe` (egui): `pkg-config`, and the
GTK/Wayland/X11 dev libraries listed in `flake.nix`.

```sh
# development shell with cargo + the GUI libs
nix develop

# inside the shell:
run          # cargo run (opens the GUI)
build        # cargo build --release
test         # cargo test
```

You can also use the flake directly:

```sh
nix run .#tuneshon
nix build .#tuneshon
```

There is a classic `shell.nix` for non-flake setups; it provides the same
`run`, `build`, and `test` helpers.

## Configuration

The app stores its settings in `~/.config/tuneshon/config.json` and writes a
log to `~/.config/tuneshon/tuneshon.log` by default. On a packaged install the
flake bakes the Nix config dir into the binary wrapper as `TUNESHON_CONFIG_DIR`,
so the app points at the right repo on first launch.

See [docbin/CONFIGURATION.md](docbin/CONFIGURATION.md) for the settings file,
environment variable, and flake module options.

## Using in your NixOS configuration

Pull the flake as an input and pre-set the Nix config dir so the app starts
already configured (no in-app setup).

```nix
{
  inputs.tuneshon = {
    url = "github:<you>/tuneshon";
    inputs.nixpkgs.follows = "nixpkgs";
  };

  # hjem (per-user) module:
  imports = [ inputs.tuneshon.hjemModules.tuneshon ];
  config.tuneshon = {
    enable = true;
    configDir = "/home/jar/nix-config";
    bootLoader = "systemd-boot"; # match your boot.nix (systemd-boot | grub | ...)
    logFile = "$HOME/.config/tuneshon/tuneshon.log";
  };
}
```

The hjem module installs the binary as a user package and exports
`TUNESHON_CONFIG_DIR`. If you use the `liijar/` app layout, mirror
`juajar/liijar/tuneshon/default.nix`, which seeds a writable `config.json` via
the dirSetup login bus so the app can still self-save.

## Documentation

- [docbin/USAGE.md](docbin/USAGE.md): how to use the GUI and CLI.
- [docbin/ARCHITECTURE.md](docbin/ARCHITECTURE.md): how the code is organized
  and how output flows from commands to the terminal.
- [docbin/CONFIGURATION.md](docbin/CONFIGURATION.md): settings, env vars, and
  flake options.
- [docbin/DEVELOPMENT.md](docbin/DEVELOPMENT.md): building, testing, packaging,
  and contributing.

## Repository layout

- `src/`: Rust application source (`cargo check`, `cargo test`).
- `gui/`: the eframe/egui interface (actions, dialogs, terminal).
- `docbin/`: documentation.
- `imgbin/`: source assets (Excalidraw + PNG) for the logo.
- `tuneshon_logo.png`: the launcher/icon image (packed into the package).
- `flake.nix`, `shell.nix`: Nix packaging and dev shells.