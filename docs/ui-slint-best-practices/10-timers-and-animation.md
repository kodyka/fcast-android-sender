# 10 — Timers & animation: replace busy tickers with `animate` and derived bindings

## Goal

Eliminate the three places the UI drives a value with a recurring 16 ms
`Timer` when an `animate` or change-driven binding is equivalent.
Specifically:

1. `lock_overlay.slint` hold-progress bar — uses a 16 ms Timer to grow
   a `hold-elapsed: duration` property. Replace with an `animate`-d
   progress property that lerps 0 → 1 over 1.5 s while the touch is
   active.
2. `connect_page.slint` long-press detection — uses a 600 ms Timer to
   "arm" a flag. Slint 1.16 ships `TouchArea.long-pressed`. Use that
   on 1.16+; keep the existing pattern as the 1.15 fallback.
3. `snapshot_countdown.slint` — uses a 1 s Timer that decrements a
   counter. A pure timer-driven countdown is fine, but the current
   shape leaks the timer (keeps running across visibility flips). Gate
   it on `visible` and stop firing once the count reaches 0.

## Findings

### F10 — `lock_overlay.slint:8–22` 16 ms Timer driving a progress bar

```slint
property <duration> hold-elapsed: 0s;
property <float> hold-progress: Math.clamp(root.hold-elapsed / 1.5s, 0.0, 1.0);

Timer {
    interval: 16ms;
    running: hold-area.pressed;
    triggered => {
        root.hold-elapsed += 16ms;
        if (root.hold-elapsed >= 1.5s) {
            Bridge.lifecycle = LifecycleMode.normal;
        }
    }
}

hold-area := TouchArea { }
changed pressed => {
    root.hold-elapsed = 0s;
}
```

This burns ~60 wakeups/sec just to grow a `duration` property the UI
already knows how to interpolate.

### F13 — `connect_page.slint:54–72` long-press emulation

```slint
property <bool> lp-armed: false;
property <duration> lp-elapsed: 0s;

Timer {
    interval: 600ms;
    running: lp-armed;
    triggered => {
        // arm the long-press
        root.show-context-menu = true;
        root.context-receiver-id = device.id;
        root.context-receiver-name = device.name;
        root.lp-armed = false;
    }
}

receiver-row-touch := TouchArea {
    changed pressed => {
        if (self.pressed) {
            root.lp-armed = true;
        } else {
            root.lp-armed = false;
        }
    }
}
```

Slint 1.16 ships `TouchArea.long-pressed` natively — emits after the
platform-defined long-press timeout, cancels on `pressed-canceled`.
The above turns into:

```slint
TouchArea {
    long-pressed => {
        root.show-context-menu      = true;
        root.context-receiver-id    = device.id;
        root.context-receiver-name  = device.name;
    }
}
```

…on 1.16+.

### F20 — `snapshot_countdown.slint:25–35` timer keeps ticking after the count reaches 0

```slint
Timer {
    interval: 1s;
    running: root.visible;
    triggered => {
        root.remaining -= 1;
        if (root.remaining <= 0) {
            root.expired();
            Bridge.lifecycle = LifecycleMode.normal;
        }
    }
}
```

The timer keeps firing as long as `root.visible` is true — the
`expired()` callback fires every second after `remaining == 0`. The
fix is one extra condition on `running`:

```slint
Timer {
    interval: 1s;
    running: root.visible && root.remaining > 0;
    triggered => {
        root.remaining = max(0, root.remaining - 1);
        if (root.remaining == 0) {
            root.expired();
            AppBridge.exit-lifecycle();
        }
    }
}
```

(`AppBridge.exit-lifecycle()` per step 07.)

## Slint docs reference

- [`timer.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/reference/timer.mdx)
  — `Timer { interval: …; running: …; triggered => { … } }`.
- [`animations.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/reference/elements/animations.mdx)
  — `animate <property> { duration: …; easing: …; }` for property
  interpolations driven by the render loop.
- `TouchArea.long-pressed` is documented at
  [`touch-area.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/reference/elements/touch-area.mdx)
  — *added in 1.16*; the doc page on this branch is the 1.17 mirror.

## Before — `lock_overlay.slint`

```slint
import { Theme } from "../theme.slint";
import { Bridge, LifecycleMode } from "../bridge.slint";

export component LockOverlay inherits Rectangle {
    callback exited;

    property <duration> hold-elapsed: 0s;
    property <float>    hold-progress:
        Math.clamp(root.hold-elapsed / 1.5s, 0.0, 1.0);

    Timer {
        interval: 16ms;
        running: hold-area.pressed;
        triggered => {
            root.hold-elapsed += 16ms;
            if (root.hold-elapsed >= 1.5s) {
                Bridge.lifecycle = LifecycleMode.normal;
                root.exited();
            }
        }
    }

    hold-area := TouchArea {
        changed pressed => {
            root.hold-elapsed = 0s;
        }
    }

    // … big lock glyph + progress bar painted from hold-progress …
    Rectangle {
        x: 0;
        y: parent.height - 4px;
        width: parent.width * root.hold-progress;
        height: 4px;
        background: Theme.accent;
    }
}
```

## After — `animate` on the progress property, callback at completion

```slint
import { Theme } from "../theme.slint";
import { AppBridge, LifecycleMode } from "../state/index.slint";

export component LockOverlay inherits Rectangle {
    callback exited;

    // 0.0 → 1.0 while the user is holding. Slint animates the change
    // for us; the actual progress is `hold-progress * 1.0` at the
    // end of the animation, which we observe via `changed`.
    property <float> hold-progress: hold-area.pressed ? 1.0 : 0.0;
    animate hold-progress {
        duration: 1.5s;
        easing: linear;
    }

    // Fires when `animate` reaches `1.0` — i.e. user has held long
    // enough. Slint emits `changed` *after* the animation completes;
    // for safety, also check pressed.
    changed hold-progress => {
        if (root.hold-progress >= 1.0 && hold-area.pressed) {
            AppBridge.exit-lifecycle();
            root.exited();
        }
    }

    hold-area := TouchArea { }

    // Progress bar reads the animated property directly.
    Rectangle {
        x: 0;
        y: parent.height - 4px;
        width: parent.width * root.hold-progress;
        height: 4px;
        background: Theme.accent;
        animate width {
            duration: 1.5s;          // matches hold-progress
            easing: linear;
        }
    }
}
```

> If you want a *snap-back* on release (current behaviour), the
> `pressed ? 1.0 : 0.0` toggle gives it for free — release flips the
> target to 0.0 and the bar animates back over 1.5 s. That's longer
> than you'd want for snap-back; add a different easing/duration on the
> "out" leg via a `states [ … ]` block (step 06):
>
> ```slint
> states [
>     held when hold-area.pressed: { hold-progress: 1.0; in { animate hold-progress { duration: 1.5s; easing: linear; } } }
>     released when !hold-area.pressed: { hold-progress: 0.0; in { animate hold-progress { duration: 200ms; easing: ease-out; } } }
> ]
> ```

The `changed hold-progress =>` block is a legitimate `changed` handler
(not a re-emit). Slint's
[`properties.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/language/coding/properties.mdx)
allows this — `changed` triggers when a value transitions through a
threshold, no callback re-emission.

## Before — `connect_page.slint` long-press emulation

```slint
TouchArea {
    changed pressed => {
        if (self.pressed) {
            root.lp-armed = true;
        } else {
            root.lp-armed = false;
        }
    }
}

Timer {
    interval: 600ms;
    running: root.lp-armed;
    triggered => {
        root.show-context-menu      = true;
        root.context-receiver-id    = device.id;
        root.context-receiver-name  = device.name;
        root.lp-armed = false;
    }
}
```

## After — native `long-pressed` (Slint 1.16+)

```slint
TouchArea {
    long-pressed => {
        root.show-context-menu      = true;
        root.context-receiver-id    = device.id;
        root.context-receiver-name  = device.name;
    }
    clicked => {
        // ordinary tap path stays
        Receivers.connect(device.id);
    }
}
```

> **Pin compatibility:** the project pins Slint 1.15.1; `long-pressed`
> doesn't exist there. Two options:
> 1. **Bump the Slint pin to 1.16+.** Other steps in this guide already
>    flag features that would benefit (`ComboBox.current-index-changed`,
>    array slice on properties). Decide once; bump everything.
> 2. **Keep the 1.15 workaround in place** until the bump. Hide it behind
>    a `LongPressTouchArea` component so the call-sites read clean:
>
>    ```slint
>    // ui/components/long_press_touch_area.slint  (1.15 fallback)
>    export component LongPressTouchArea inherits TouchArea {
>        in property <duration> long-press-threshold: 600ms;
>        callback long-pressed();
>
>        property <bool> armed: self.pressed;
>        property <duration> elapsed: 0s;
>
>        // Single Timer instance, scoped to the component.
>        Timer {
>            interval: 16ms;
>            running: root.armed && root.elapsed < root.long-press-threshold;
>            triggered => {
>                root.elapsed += 16ms;
>                if (root.elapsed >= root.long-press-threshold) {
>                    root.long-pressed();
>                    root.armed = false;
>                }
>            }
>        }
>        changed pressed => {
>            root.elapsed = 0s;
>            root.armed = self.pressed;
>        }
>    }
>    ```
>
>    Call-site:
>
>    ```slint
>    LongPressTouchArea {
>        long-pressed => {
>            root.show-context-menu      = true;
>            root.context-receiver-id    = device.id;
>            root.context-receiver-name  = device.name;
>        }
>    }
>    ```
>
>    Same Timer cost as today, but isolated and consistent across the
>    codebase.

## Before — `snapshot_countdown.slint:25–35`

```slint
Timer {
    interval: 1s;
    running: root.visible;
    triggered => {
        root.remaining -= 1;
        if (root.remaining <= 0) {
            root.expired();
            Bridge.lifecycle = LifecycleMode.normal;
        }
    }
}
```

## After — auto-stop on completion + go through `exit-lifecycle`

```slint
Timer {
    interval: 1s;
    running: root.visible && root.remaining > 0;
    triggered => {
        root.remaining = max(0, root.remaining - 1);
        if (root.remaining == 0) {
            root.expired();
            AppBridge.exit-lifecycle();
        }
    }
}
```

## A note on animation cost

Slint's `animate` block runs *inside* the render loop — there is no
extra wakeup beyond the frame the GPU was already going to draw. A 60
fps render loop already wakes the UI thread; piggy-backing a property
interpolation on it is effectively free. A 16 ms `Timer` is **not**
free: it wakes the event loop independently, regardless of whether a
new frame is needed. Treating `animate` as the default for "smooth
visual" effects is the rule; `Timer` is for **discrete** ticks like
"every 1 s emit a callback".

## Migration

1. Rewrite `lock_overlay.slint` to use `animate hold-progress`. Verify
   "release before 1.5 s" cancels the exit (current behaviour).
2. Decide on the Slint pin bump for `long-pressed`. If staying on 1.15,
   add `LongPressTouchArea` and migrate `connect_page.slint` to it.
3. Add the `&& root.remaining > 0` guard to
   `snapshot_countdown.slint`.
4. Audit other Timers in the repo (`grep -rn 'Timer {' ui/`):
   - `connect_page.slint` long-press (handled above)
   - `lock_overlay.slint` (handled above)
   - `snapshot_countdown.slint` (handled above)
   - That's all — the std-widgets stable use of `Timer` is fine.

### Per-file checklist

| File                                    | Change                                                  |
| --------------------------------------- | ------------------------------------------------------- |
| `ui/components/lock_overlay.slint`      | 16 ms Timer → `animate hold-progress` + `changed`       |
| `ui/components/snapshot_countdown.slint`| Add `&& root.remaining > 0` guard                       |
| `ui/pages/connect_page.slint`           | Long-press via `LongPressTouchArea` or `long-pressed`   |
| `ui/components/long_press_touch_area.slint` | NEW (only if staying on Slint 1.15)                 |

## Out of scope

- Migrating recording-elapsed tick to Rust-side push. The current
  shape (Rust ticks `recording-elapsed-s` every second via
  `slint::Timer`) is correct.
- Bigger animation choreography (page transitions, etc.).
- A FPS budget tool. The `animate` rewrite buys the budget back.

## Acceptance

- [ ] No `Timer { interval: 16ms; … }` exists outside
      `LongPressTouchArea` (and only on 1.15 pin).
- [ ] `LockOverlay` snaps back smoothly to 0% on release in
      `slint-viewer`.
- [ ] `SnapshotCountdown` stops firing once `remaining == 0`.
- [ ] Power profile (manual): pressing-and-holding the lock screen
      does not generate the previous 60 wakeups/sec churn — verify with
      `dumpsys batterystats` on Android.
