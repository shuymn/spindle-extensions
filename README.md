# spindle-extensions

Personal stdio JSONL extensions for [spindle](https://github.com/shuymn/spindle).

The spindle kernel and `spindle-extension-sdk` live in the main spindle repository.
This repository holds provider and workflow extensions under `extensions/<name>/`.

## Extensions

- `extensions/aerospace/` — AeroSpace IPC, workspace/mode/layout snapshots, workspace focus
- `extensions/sketchybar/` — SketchyBar Mach IPC and generic message send
- `extensions/workspace-indicator/` — AeroSpace state to SketchyBar message workflow

## Layout

```text
spindle-extensions/
  extensions/
    aerospace/
    sketchybar/
    workspace-indicator/
```

Extensions depend on `spindle-extension-sdk` from the `shuymn/spindle` Git repository.

## Setup

Clone this repository:

```bash
git clone https://github.com/shuymn/spindle-extensions.git
cd spindle-extensions
```

```bash
task build
task test
```

Build release binaries before installing extensions:

```bash
task build:release
```

Release binaries land in `target/release/`:

- `spindle-aerospace`
- `spindle-sketchybar`
- `spindle-workspace-indicator`

Install by manifest path or extension directory from a spindle checkout or installed `spindle` binary:

```bash
spindle install --trust-runtime /path/to/spindle-extensions/extensions/aerospace
spindle install --trust-runtime /path/to/spindle-extensions/extensions/sketchybar/extension.json
```

## License

MIT
