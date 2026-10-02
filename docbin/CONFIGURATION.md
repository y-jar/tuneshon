# Configuration

The app persists user settings and reads a baked-in default from the install.
This page covers the config file, the environment variable, and the flake
module options.

## Config file

Located at `~/.config/tuneshon/config.json`:

```json
{
  "config_dir": "/home/jar/nix-config",
  "log_file": "/home/jar/.config/tuneshon/tuneshon.log",
  "boot_loader": "systemd-boot"
}
```

- `config_dir`: the Nix config repository the app operates on. It is the
  working directory for `git`/`nix`/`nh` and the repo root for `git add`.
- `log_file`: where the app appends command output.
- `boot_loader`: displayed for reference (`systemd-boot`, `grub`, `efi`).

Edit these in the Settings dialog, or the app writes a default on first run.

## Environment variable

`TUNESHON_CONFIG_DIR` sets the default `config_dir`. On a packaged install the
flake bakes it into the binary wrapper, so the app points at the right repo on
first launch without in-app setup. It takes precedence over any persisted value
in `config.json`.

## Flake outputs

The flake exposes:

- `packages.<system>.tuneshon` (also `.default`): the app, baked with
  `configDir = "/etc/nixos"`.
- `devShells.<system>.default`: a dev shell with cargo and the GUI libraries.
- `lib.mkPackage { pkgs, configDir }`: build a tuneshon package baked with a
  specific config dir, for hosts that need their own default.
- `hjemModules.tuneshon` (also `.default`): a hjem user module.

### hjem module options

```nix
config.tuneshon = {
  enable = true;
  configDir = "/home/jar/nix-config";
  bootLoader = "systemd-boot";
  logFile = "$HOME/.config/tuneshon/tuneshon.log";
};
```

When enabled, the module installs the package (built via `lib.mkPackage` with
your `configDir`) and exports `TUNESHON_CONFIG_DIR` as a session variable.

## liijar integration pattern

If you use the `juajar/liijar` app layout in your NixOS config, mirror
`juajar/liijar/tuneshon/default.nix`. It installs the package and seeds a
writable `config.json` through the dirSetup `.profile` login bus, so the app
can still self-save later. See the tuneshon README for the module form.