# spindle-clock-extension

Clock label workflow extension for spindle.

This package replaces the former SketchyBar shell plugin:

```sh
sketchybar --set "$NAME" label="$(LC_TIME=C date '+%m/%d %a %H:%M:%S')"
```

`clock.render` reads the target item from `item`, `name`, or `NAME` and emits a
`clock.sketchybar.message.requested` event. The registered route sends that event
to `sketchybar.message.send` with the `sketchybar.ui.write` capability.

Periodic execution remains external to spindle. Trigger `clock.render` from the
same scheduler that previously ran the SketchyBar plugin.
