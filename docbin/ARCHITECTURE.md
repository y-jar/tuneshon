# Architecture

tuneshon builds shell commands, runs them as subprocesses, and streams their
output back into an egui interface. Everything is plain Rust using tokio for
concurrency.

## Module map

- `main.rs`: entry point; parses the CLI, loads `AppConfig`, dispatches to the
  GUI or the headless CLI runner.
- `config.rs`: the `~/.config/tuneshon/config.json` model, load/save, and the
  `TUNESHON_CONFIG_DIR` env override.
- `runner.rs`: runs `sh -c <cmdline>` in the config dir, reads stdout/stderr on
  tokio tasks, and forwards them as `Event`s to a `Sink`.
- `generations.rs`: parses `nh os info` output into `Generation` rows; detects
  the current generation from the profile symlink.
- `text.rs`: shared ANSI-stripping helper (used by the terminal and
  generations parsing).
- `gui/mod.rs`: the eframe `App`; owns the panels, event loop, and command
  lifecycle.
- `gui/actions.rs`: builds command strings for each action (preview vs apply).
- `gui/dialogs.rs`: modal dialogs (settings, flake picker, confirm, help).
- `gui/terminal.rs`: the scrollable, per-line colored terminal buffer.

## Event flow

The GUI and the runner communicate over a crossbeam channel. The runner pushes
events, and the GUI drains them in `update()`:

```
command process
   | stdout / stderr lines
   v
runner.rs  --Event-->  Sink  --tx-->  channel  --rx-->  App::drain_events  --> Terminal
```

`Event` has three kinds: `Out` (a stdout line), `Err` (a stderr line), and
`Done(exit_code)` (the process finished). A fourth, `Generations`, is pushed by
an async fetch for the generations panel. Lines are ANSI-stripped and colored
by the terminal's `classify`.

## Action lifecycle

Each action has two phases:

1. **Preview** (`nh os build -e none ...`), run as the unprivileged user.
   The `git pull` / `git add .` / `nix flake update` step, if any, also runs
   here.
2. **Apply** (`nh os switch|boot|test ...`), run only after the user confirms.
   The whole apply chain is wrapped in a single `pkexec sh -c '...'` so polkit
   prompts for the password exactly once.

Flow:

```
click action
  -> spawn preview  (running = true)
  -> on Done(0): stash apply, open confirm dialog
  -> Apply: spawn apply (single pkexec)
  -> Cancel: print "[cancelled]", back to idle
  -> on Done: refresh generations
```

Pure input updates (`update packages`, `update app`, `specify update?`) skip
the build and confirm steps entirely.

## Why a single pkexec wrapper

`nh` internally runs many elevated sub-steps. If each called `pkexec` on its
own, the user would be prompted many times. Instead `build_apply` wraps the
whole chain in one `pkexec sh -c '...'` and runs `nh` with `-e none`, so the
entire apply needs one password prompt.

## Repainting

egui only repaints on input. While a command is running or generations are
loading, `App::update()` calls `ctx.request_repaint_after(50ms)` so the
terminal streams without the user moving the mouse.

## Fonts

`install_system_fonts` loads the system monospace and a Unicode/sans fallback
via `fc-match` and registers them with egui, so box-drawing and arrow glyphs
output by `nh` render instead of missing-glyph boxes.