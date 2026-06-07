# workspace-indicator

Workspace indicator workflow package for spindle.

This package has a stdio JSONL host entrypoint, but it does not talk to `AeroSpace` or
`SketchyBar` directly. It owns the projection that used to live in local
SketchyBar scripts: workspace labels, colors, status labels, and cache keys.

It connects provider extensions through routes, owns workspace render scheduling,
and produces generic provider requests from action output:

- `aerospace` emits domain events and exposes state/control actions.
- `workspace-indicator` registers source-bound routes from its host.
- `workspace-indicator.workspaces.schedule` records render intents and returns immediately.
- The long-lived host owns debounce timing and latest-wins generation state.
- After the debounce window, the scheduler invokes `aerospace.workspace.snapshot`
  through spindle using a continuation handle.
- `workspace-indicator` projects `AeroSpace` snapshots into
  produced `workspace-indicator.sketchybar.message.requested` events.
- `sketchybar` owns the actual UI write action.

Routes in this package grant only the capabilities needed by the target action
and continuation-backed work. Workspace schedule routes grant
`aerospace.state.read` and `sketchybar.ui.write`; click routes grant
`aerospace.window.control`; SketchyBar output routes grant `sketchybar.ui.write`.

Before scheduling or producing a request, the host reads
`ActionContext::extension()` and checks that required provider/output surfaces
are available. `workspace-indicator.workspaces.schedule` also requires
`ActionContext::continuation()` because delayed work must go back through spindle.

The important path is:

```text
aerospace.workspace.changed / aerospace.monitor.changed
  -> workspace-indicator.workspaces.schedule
  -> workspace-indicator-owned latest-wins scheduler
  -> aerospace.workspace.snapshot
  -> workspace-indicator.workspaces.render
  -> workspace-indicator.sketchybar.message.requested
  -> sketchybar.message.send

sketchybar.workspace.clicked -> aerospace.workspace.focus
  -> aerospace.workspace.snapshot
  -> workspace-indicator.workspaces.render
  -> workspace-indicator.sketchybar.message.requested
  -> sketchybar.message.send
```

## Canonical surface

Inspect runtime registration before writing route policy or downstream packager config:

```sh
spindle-workspace-indicator register | jq .
spindle extension validate extensions/workspace-indicator/extension.json
```

Key names from this extension:

| Kind | Name |
|------|------|
| produced event | `workspace-indicator.sketchybar.message.requested` |
| action | `workspace-indicator.workspaces.schedule` |
| action | `workspace-indicator.workspaces.render` |
| action | `workspace-indicator.status.render` |

Route policy must reference `workspace-indicator.sketchybar.message.requested`, not legacy names such as `workspace-indicator.rendered`.

Obsolete scheduler generations are suppressed before invoking
`aerospace.workspace.snapshot`, so obsolete workspace intents do not read
AeroSpace state or produce SketchyBar message requests. The workspace scheduler
no longer relies on provider-side `settle_ms`; AeroSpace remains a narrow state
provider.
