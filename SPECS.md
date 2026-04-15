# Fade — Open Design Notes

## Animated image formats (GIF, APNG, WebP anim)

**Status:** deferred — no implementation yet.

**The problem:**  
GIF and other animated formats don't fit cleanly into the static image pipeline. The input context changes what the UI should offer:

- **Video → GIF**: the interesting controls are frame rate, palette size, dither mode, loop count, and a time-range selector (same as the video trim already exists). Output size is the main tradeoff.
- **GIF → GIF**: palette/dither re-encoding, frame delay editing, resize, crop. Basically a lightweight animation editor.
- **GIF → video**: straightforward — pick a codec and let the existing video pipeline handle it. Probably just route through the video options panel.
- **APNG / animated WebP**: same shape as GIF but different codec tradeoffs.

**What the UI should do (when we get to it):**  
Detect on file selection whether the selected image is animated (`get_file_info` can expose a `frame_count > 1` flag). If animated, swap the right-panel options to an "Animated Image" panel instead of the static Image panel. That panel should surface: output format (GIF / APNG / animated WebP / MP4), frame rate, loop count, and palette/dither controls when the target is GIF.

**Rust side:**  
ffmpeg already handles all of these conversions. The main GIF-specific thing is the palette — `palettegen` + `paletteuse` filter chain in ffmpeg is required for good-quality GIF output from video. Without it the output looks terrible.

**Not blocked on anything — just not a priority yet.**
