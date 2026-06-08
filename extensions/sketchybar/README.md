# spindle-sketchybar-extension

SketchyBar capability extension for spindle.

This package owns SketchyBar Mach IPC and UI write actions. It does not read
AeroSpace state and does not know workspace-indicator rendering rules.

`sketchybar.message.send` accepts generic `SketchyBar` command arguments from
routed events.

## Write cache

When routed events include `cache_key` and `cache_value`, the extension skips
Mach IPC if `{cache_key}.state` in the cache directory already matches
`cache_value`. This optimizes duplicate snapshot delivery; it does not verify
what SketchyBar currently displays.

The extension invalidates all `*.state` files when:

- the SketchyBar Mach endpoint transitions from unavailable to available
  (start or restart), or
- `BAR_NAME` changes.

On Mach send failure, the affected cache entry is deleted.

## Cache directory

Resolved in this order:

1. `SPINDLE_SKETCHYBAR_STATE_DIR` — sketchybar-specific override
2. `SPINDLE_STATE_DIR` — generic spindle state (export from daemon launch env)
3. `$TMPDIR/sketchybar-cache/` (or `/tmp/sketchybar-cache/`)

The extension does not read other extensions' environment variables. Co-locate
cache with aerospace or workspace-indicator state by setting
`SPINDLE_SKETCHYBAR_STATE_DIR` or `SPINDLE_STATE_DIR` in launch config.

Export `SPINDLE_STATE_DIR` from the spindle daemon launch environment so cache
files live alongside spindle state. The spindle kernel does not inject
extension-specific environment variables.

## CLI

```bash
# Register extension surface
spindle-sketchybar register

# Clear all write-cache entries
spindle-sketchybar invalidate-cache --state-dir "$SPINDLE_STATE_DIR"

# Clear one cache key
spindle-sketchybar invalidate-cache --state-dir "$SPINDLE_STATE_DIR" --key workspace-indicator.workspaces
```

Use `invalidate-cache` in bootstrap scripts after SketchyBar restarts when
emitting the same snapshot twice before the extension has observed the endpoint
transition.
