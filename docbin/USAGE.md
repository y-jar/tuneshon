# Usage

tuneshon opens a GUI by default. Running with no arguments launches the
window; use the CLI subcommands below for headless operation.

## GUI

The window has three areas:

- **Left panel: Actions + Update inputs.**
- **Center: Output terminal.**
- **Right panel: Generations.**

### Actions

Each action first runs a non-elevated `nh os build`. Watch the terminal to
confirm the config builds. If it succeeds, you are asked to Apply or Cancel.
Applying runs the elevated switch/boot/test through a single `pkexec`, which
prompts once for your password via the polkit agent.

| Button | What it does |
| --- | --- |
| full update | `git pull`, build, then switch + boot (applies on next reboot). For systems you do not tinker with. |
| update | `git add .`, build, then switch + boot. |
| test | `git add .`, build, then test-activate. Does not change the boot default, so it is safe to try. |
| update after restart | `git add .`, build, then boot (applies on next reboot). |
| specify update? | Open a picker to choose which flake inputs to run `nix flake update` on. No switch. |
| specify update & update | Pick flake inputs, `nix flake update`, build, then switch. |

For the two "specify" actions, choose one or more inputs in the picker, then
press Update.

### Update inputs

Quick, non-elevated `nix flake update` buttons that do not do a full build:

- **packages**: `nixpkgs`
- **addons**: `wall-jar`, `icon-jar`, `mcskins-jar`, `shelljar`, `tuneshon`
- **kernel**: `nix-cachyos-kernel`
- **update app**: update the tuneshon input and build. Use this when you are
  told to update the app; it does not pull or `git add` the config repo.

### Terminal

- Output streams here in a scrollable, colored view. Errors are red, warnings
  amber, success lines green, and muted/status lines dim.
- **follow logs** scrolls to the newest line.
- **verbose logs: on/off** (default off) adds extra activation logs on
  switch/boot/test for a more detailed view.

### Generations

The right panel lists system generations from `nh os info`, newest first. The
current generation is highlighted in green.

### Settings

The gear icon opens settings for the Nix config dir, log file, and boot loader.
Use the folder buttons to browse. Save writes the config; Reset defaults it.

## CLI

The CLI runs the same commands headlessly. Each subcommand runs the preview
followed by the apply in one shell (it cannot show the confirmation gate, so
the elevated step runs directly; `pkexec` will prompt for your password).

```
tuneshon full-update
tuneshon update [--dir <path>]
tuneshon test [--dir <path>]
tuneshon boot [--dir <path>]
tuneshon flake-update [inputs...]
tuneshon flake-update-switch [inputs...]
tuneshon --gui
```

If `inputs` are omitted, `flake-update` updates every input present in
`flake.lock`. Use `--help` on any subcommand for details.