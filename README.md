app name: tuneshon meaning [update]

- scjar = source code holder, that is where all the main code goes
- docbin = documentation bin
- shell.nix = Nix shell configuration


goals for app.
serve

## Using in your NixOS configuration

Pull the flake as an input and pre-set the Nix config dir so looks correct on
first launch (no in-app setup).

```nix
{
  inputs.tuneshon = {
    url = "github:<you>/tuneshon";
    inputs.nixpkgs.follows = "nixpkgs";
  };

  # hjem (per-user) module:
  # imports = [ inputs.tuneshon.hjemModules.tuneshon ];
  # config.tuneshon = {
  #   enable = true;
  #   configDir = "/home/jar/nix-config";
  #   bootLoader = "systemd-boot"; # match your boot.nix (systemd-boot | grub | ...)
  #   logFile = "$HOME/.config/tuneshon/tuneshon.log";
  # };
}
```

The hjem module installs the binary as a user package and exports
`TUNESHON_CONFIG_DIR`, which the app reads as the default config dir
(`src/config.rs`). If you already use the `liijar/` app layout, mirror
`juajar/liijar/tuneshon/default.nix` (writes a writable `config.json` via the
dirSetup login bus so the app can still self-save).