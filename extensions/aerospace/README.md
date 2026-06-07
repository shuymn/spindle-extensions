# spindle-aerospace-extension

AeroSpace capability extension for spindle.

This package owns AeroSpace IPC and control actions such as
`aerospace.workspace.focus`. It also exposes state snapshot actions such as
`aerospace.workspace.snapshot`, `aerospace.mode.snapshot`, and
`aerospace.layout.snapshot`.

It does not write SketchyBar UI.

## External callbacks

When AeroSpace config callbacks emit spindle events (for example `aerospace.workspace.changed`), do not assume `spindle` is on `PATH`. launchd and other minimal environments often omit user shell paths.

Prefer an explicit binary path:

```sh
SPINDLE_BIN="${SPINDLE_BIN:-/absolute/path/to/spindle}"
"$SPINDLE_BIN" send --request ...
```

Fail loudly when the binary is missing instead of redirecting stderr/stdout to `/dev/null`. Packaged setups should set `SPINDLE_BIN` to the store path, for example `/nix/store/.../bin/spindle`.

Workspace click rendering no longer depends on these callbacks: `aerospace.workspace.focus` returns a workspace snapshot that spindle routes to `workspace-indicator`.
