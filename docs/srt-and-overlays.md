# SRT Sources & Image Overlays

## Adding SRT sources

1. Open **Settings > SRT Sources & Overlays**
2. Tap **+ Add SRT Source**
3. Enter the SRT URL (e.g. `srt://relay.example:9710?mode=caller`)
4. Adjust latency (default: 2000 ms)
5. Optionally set a stream ID

## Adding image overlays

1. In the SRT source card, tap **+ Add Overlay**
2. Enter the image file path or URL
3. Adjust position (X, Y), size (Width, Height), alpha, and z-order
4. The composition preview shows a schematic layout

## Overlay parameters

| Parameter | Range | Description |
|-----------|-------|-------------|
| X, Y | -1920..3840 | Position offset from top-left |
| Width, Height | 0..1920 | 0 = use original image size |
| Alpha | 0.0..1.0 | Opacity |
| Z-order | 0..99 | Higher = drawn on top |

## Auto-reconnection

SRT sources automatically reconnect on connection loss:
- Default: 5 retries with 2-second delay
- Configurable per-source via `max_retries` and `retry_delay_ms`
- Connection state is shown in the UI (Disconnected / Connecting / Connected / Reconnecting / Error)
