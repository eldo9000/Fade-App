# Format Controls Spec

Per-format controls for Fade. This file is the source-of-truth for the **frontend → backend contract** — each option field below is declared in the options `$state` block in `src/App.svelte` and rendered (scoped to the formats it applies to) in the corresponding `*Options.svelte` component. A downstream agent wires these into ffmpeg / ImageMagick invocations.

Every non-`todo` format in `FORMAT_GROUPS` was audited. Controls already present are listed as **existing**; controls added in this pass are listed as **added**.

---

## 1. Audit matrix

### Audio (`src/lib/AudioOptions.svelte`)

Shared across all audio formats: `bitrate`, `sample_rate`, `trim_start`, `trim_end`, `normalize_loudness` (+ LUFS / true-peak sub-options), `dsp_limiter_db`, `dsp_highpass_freq`, `dsp_lowpass_freq`, `dsp_stereo_width`.

| Format | Existing (shared) | Added (format-specific) |
| --- | --- | --- |
| mp3  | bitrate, sample_rate | `mp3_bitrate_mode` (cbr/vbr), `mp3_vbr_quality` (0–9), `channels` |
| wav  | sample_rate          | `bit_depth` (16/24/32), `channels` |
| flac | sample_rate          | `flac_compression` (0–8), `bit_depth` (16/24) |
| ogg  | bitrate, sample_rate | `ogg_bitrate_mode` (vbr/cbr/abr), `ogg_vbr_quality` (-1..10) |
| aac  | bitrate, sample_rate | `aac_profile` (lc/he/hev2), `channels` |
| opus | bitrate, sample_rate | `opus_application` (audio/voip/lowdelay), `opus_vbr`, `channels` |
| m4a  | bitrate, sample_rate | `m4a_subcodec` (aac/alac), `bit_depth` (when ALAC) |
| wma  | bitrate, sample_rate | `wma_mode` (standard/pro/lossless) |
| aiff | sample_rate          | `bit_depth` (16/24/32), `channels` |
| alac | sample_rate          | `bit_depth` (16/24/32), `channels` |
| ac3  | sample_rate          | `ac3_bitrate` (192/384/448/640), `channels` |
| dts  | sample_rate          | `dts_bitrate` (754/1510), `channels` |

Bitrate field is hidden for lossless formats (flac/wav/aiff/alac, m4a+alac, ac3/dts which use their own bitrate fields, and mp3/ogg when VBR is selected).

### Video (`src/lib/VideoOptions.svelte`)

Shared: `codec`, `resolution`, `bitrate` (audio), `sample_rate`, `remove_audio`, `extract_audio`, `audio_format`, `trim_start`, `trim_end`.

| Format | Existing | Added |
| --- | --- | --- |
| mp4  | codec, resolution | `crf`, `preset`, `h264_profile`, `pix_fmt`, `tune`, `frame_rate`, codec-dependent `vp9_speed`/`av1_speed` |
| mov  | codec, resolution | same as mp4 |
| webm | codec, resolution | same as mp4 + `webm_bitrate_mode` |
| mkv  | codec, resolution | same as mp4 + `mkv_subtitle` |
| avi  | codec, resolution | same as mp4 + `avi_video_bitrate` |
| gif  | —                 | `gif_width`, `gif_fps`, `gif_loop`, `gif_palette_size`, `gif_dither` (replaces codec/CRF/preset which don't apply) |

Codec-specific controls auto-hide: `h264_profile` / `tune` only when codec is `h264`/`h265`; `vp9_speed` only when `vp9`; `av1_speed` only when `av1`; all CRF/preset controls suppressed when codec is `copy`.

### Image (`src/lib/ImageOptions.svelte`)

Shared: `quality` (lossy only), `resize_mode`, `resize_percent`, `resize_width`, `resize_height`, `crop_*`, `rotation`, `flip_h`, `flip_v`, `auto_rotate`.

| Format | Existing | Added |
| --- | --- | --- |
| jpeg | quality | `jpeg_chroma` (420/422/444), `jpeg_progressive` |
| png  | —       | `png_compression` (0–9), `png_color_mode`, `png_interlaced` |
| webp | quality | `webp_lossless`, `webp_method` (0–6) |
| tiff | —       | `tiff_compression`, `tiff_bit_depth`, `tiff_color_mode` |
| bmp  | —       | `bmp_bit_depth` |
| avif | quality | `avif_speed` (0–10), `avif_chroma` |

### Data (`src/lib/DataOptions.svelte`)

| Format | Existing | Added |
| --- | --- | --- |
| json | pretty_print | — |
| csv  | csv_delimiter | — |
| tsv  | — | — |
| xml  | — | — |
| yaml | — | — |

All data formats are plain-text serialization; no additional codec-like controls are warranted.

### Document (`src/lib/FormatPicker.svelte`)

| Format | Existing | Added |
| --- | --- | --- |
| html, pdf, txt, md | output_format only | — |

Document conversion is primarily pandoc-driven and doesn't need per-format knobs beyond the picker at this stage.

### Archive (`src/lib/ArchiveOptions.svelte` — new)

| Format | Existing | Added |
| --- | --- | --- |
| zip | — | `archive_compression` (0–9) |
| tar | — | — (no compression field) |
| gz  | — | `archive_compression` |
| 7z  | — | `archive_compression` |

The old inline `FormatPicker` in App.svelte has been replaced with a dedicated `ArchiveOptions` component.

---

## 2. Implementation spec — new option fields

All fields are snake_case, live on the category options object in `src/App.svelte`, and have sensible defaults.

### Audio — `audioOptions`

### channels
- Type: `String`
- Enum: `"source" | "mono" | "stereo" | "joint" | "5.1"`
- Default: `"source"`
- Applies to formats: mp3, wav, aiff, aac, opus, alac, ac3, dts
- Widget: segmented buttons
- UI location: AudioOptions.svelte
- ffmpeg: `-ac 1` (mono), `-ac 2` (stereo / joint), `-ac 6` (5.1), omit for source. For mp3 joint-stereo add `-joint_stereo 1`.

### bit_depth
- Type: `u32`
- Enum: `16 | 24 | 32` (wav/aiff/alac may also emit 32-bit float)
- Default: `16`
- Applies to formats: wav, aiff, flac (16/24 only), alac, m4a (when ALAC)
- Widget: segmented buttons
- UI location: AudioOptions.svelte
- ffmpeg: `-sample_fmt s16 / s24 / s32` (or `flt` for 32-bit float on wav/aiff).

### mp3_bitrate_mode
- Type: `String`
- Enum: `"cbr" | "vbr"`
- Default: `"cbr"`
- Applies to formats: mp3
- Widget: segmented (2-way)
- UI location: AudioOptions.svelte
- ffmpeg: CBR → `-b:a <bitrate>k`; VBR → `-q:a <mp3_vbr_quality>`.

### mp3_vbr_quality
- Type: `u32`
- Range: `0..=9` (0 best, 9 smallest)
- Default: `2`
- Applies to formats: mp3 (only when `mp3_bitrate_mode === "vbr"`)
- Widget: slider
- UI location: AudioOptions.svelte
- ffmpeg: `-q:a <value>` on libmp3lame.

### flac_compression
- Type: `u32`
- Range: `0..=8`
- Default: `5`
- Applies to formats: flac
- Widget: slider
- UI location: AudioOptions.svelte
- ffmpeg: `-compression_level <value>`.

### ogg_bitrate_mode
- Type: `String`
- Enum: `"vbr" | "cbr" | "abr"`
- Default: `"vbr"`
- Applies to formats: ogg
- Widget: segmented (3-way)
- UI location: AudioOptions.svelte
- ffmpeg: VBR → `-q:a <ogg_vbr_quality>`; CBR → `-b:a <bitrate>k -minrate -maxrate`; ABR → `-b:a <bitrate>k`.

### ogg_vbr_quality
- Type: `i32`
- Range: `-1..=10`
- Default: `5`
- Applies to formats: ogg (only when `ogg_bitrate_mode === "vbr"`)
- Widget: slider
- UI location: AudioOptions.svelte
- ffmpeg: `-q:a <value>` on libvorbis.

### aac_profile
- Type: `String`
- Enum: `"lc" | "he" | "hev2"`
- Default: `"lc"`
- Applies to formats: aac
- Widget: vertical segmented
- UI location: AudioOptions.svelte
- ffmpeg: `-profile:a aac_low | aac_he | aac_he_v2` (libfdk_aac) or emulate with `-aac_coder` on native encoder.

### opus_application
- Type: `String`
- Enum: `"audio" | "voip" | "lowdelay"`
- Default: `"audio"`
- Applies to formats: opus
- Widget: segmented
- UI location: AudioOptions.svelte
- ffmpeg: `-application <value>` on libopus.

### opus_vbr
- Type: `bool`
- Default: `true`
- Applies to formats: opus
- Widget: checkbox
- UI location: AudioOptions.svelte
- ffmpeg: `-vbr on | off`.

### m4a_subcodec
- Type: `String`
- Enum: `"aac" | "alac"`
- Default: `"aac"`
- Applies to formats: m4a
- Widget: segmented (2-way)
- UI location: AudioOptions.svelte
- ffmpeg: `-c:a aac` or `-c:a alac`.

### wma_mode
- Type: `String`
- Enum: `"standard" | "pro" | "lossless"`
- Default: `"standard"`
- Applies to formats: wma
- Widget: vertical segmented
- UI location: AudioOptions.svelte
- ffmpeg: `-c:a wmav2` (standard), `-c:a wmapro` (pro), `-c:a wmalossless` (lossless).

### ac3_bitrate
- Type: `u32`
- Enum: `192 | 384 | 448 | 640` (kbps)
- Default: `448`
- Applies to formats: ac3
- Widget: segmented
- UI location: AudioOptions.svelte
- ffmpeg: `-b:a <value>k` on ac3.

### dts_bitrate
- Type: `u32`
- Enum: `754 | 1510` (kbps)
- Default: `1510`
- Applies to formats: dts
- Widget: segmented
- UI location: AudioOptions.svelte
- ffmpeg: `-b:a <value>k` on dca.

---

### Video — `videoOptions`

### crf
- Type: `u32`
- Range: `0..=51`
- Default: `23`
- Applies to formats: mp4, mov, webm, mkv, avi (when codec ≠ `copy`)
- Widget: slider
- UI location: VideoOptions.svelte
- ffmpeg: `-crf <value>` (x264/x265/libvpx-vp9/libaom-av1).

### preset
- Type: `String`
- Enum: `"ultrafast" | "fast" | "medium" | "slow" | "veryslow"`
- Default: `"medium"`
- Applies to formats: mp4, mov, webm, mkv, avi (when codec ≠ `copy`)
- Widget: vertical segmented
- UI location: VideoOptions.svelte
- ffmpeg: `-preset <value>`.

### h264_profile
- Type: `String`
- Enum: `"baseline" | "main" | "high"`
- Default: `"high"`
- Applies to formats: mp4/mov/mkv/avi when codec is `h264` or `h265`
- Widget: segmented
- UI location: VideoOptions.svelte
- ffmpeg: `-profile:v <value>`.

### pix_fmt
- Type: `String`
- Enum: `"yuv420p" | "yuv422p" | "yuv444p"`
- Default: `"yuv420p"`
- Applies to formats: mp4, mov, webm, mkv, avi (when codec ≠ `copy`)
- Widget: segmented
- UI location: VideoOptions.svelte
- ffmpeg: `-pix_fmt <value>`.

### tune
- Type: `String`
- Enum: `"none" | "film" | "animation" | "grain"`
- Default: `"none"`
- Applies to formats: mp4/mov/mkv/avi when codec is `h264` or `h265`
- Widget: segmented
- UI location: VideoOptions.svelte
- ffmpeg: `-tune <value>` (omit if `none`).

### frame_rate
- Type: `String`
- Enum: `"original" | "24" | "25" | "30" | "60"`
- Default: `"original"`
- Applies to formats: all video except gif (which uses `gif_fps`)
- Widget: segmented
- UI location: VideoOptions.svelte
- ffmpeg: `-r <n>` when not `"original"`.

### webm_bitrate_mode
- Type: `String`
- Enum: `"crf" | "cbr" | "cvbr"`
- Default: `"crf"`
- Applies to formats: webm
- Widget: segmented
- UI location: VideoOptions.svelte
- ffmpeg: CRF → `-crf <crf> -b:v 0`; CBR → `-minrate -maxrate -b:v <n>`; CVBR → `-b:v <n> -maxrate <n*1.5>`.

### vp9_speed
- Type: `u32`
- Range: `0..=5`
- Default: `1`
- Applies to formats: any video when codec is `vp9`
- Widget: slider
- UI location: VideoOptions.svelte
- ffmpeg: `-deadline good -cpu-used <value>`.

### av1_speed
- Type: `u32`
- Range: `0..=10`
- Default: `8`
- Applies to formats: any video when codec is `av1`
- Widget: slider
- UI location: VideoOptions.svelte
- ffmpeg: `-cpu-used <value>` (libaom-av1) or `-preset <value>` (libsvtav1, inverted sense).

### mkv_subtitle
- Type: `String`
- Enum: `"none" | "copy" | "burn"`
- Default: `"copy"`
- Applies to formats: mkv
- Widget: segmented
- UI location: VideoOptions.svelte
- ffmpeg: none → `-sn`; copy → `-c:s copy`; burn → `-vf subtitles=<input>` on video filter chain.

### avi_video_bitrate
- Type: `u32` (kbps)
- Enum: `1000 | 4000 | 8000 | 20000`
- Default: `4000`
- Applies to formats: avi
- Widget: segmented
- UI location: VideoOptions.svelte
- ffmpeg: `-b:v <value>k`.

### gif_width
- Type: `u32 | "original"`
- Enum: `320 | 480 | 640 | "original"`
- Default: `480`
- Applies to formats: gif
- Widget: segmented
- UI location: VideoOptions.svelte
- ffmpeg: `-vf scale=<value>:-1:flags=lanczos` (omit when `"original"`).

### gif_fps
- Type: `u32 | "original"`
- Enum: `5 | 10 | 15 | "original"`
- Default: `10`
- Applies to formats: gif
- Widget: segmented
- UI location: VideoOptions.svelte
- ffmpeg: inserted into filtergraph as `fps=<value>`.

### gif_loop
- Type: `String`
- Enum: `"infinite" | "once" | "none"`
- Default: `"infinite"`
- Applies to formats: gif
- Widget: segmented
- UI location: VideoOptions.svelte
- ffmpeg: `-loop 0` (infinite), `-loop 1` (once), `-loop -1` (none).

### gif_palette_size
- Type: `u32`
- Enum: `32 | 64 | 128 | 256`
- Default: `256`
- Applies to formats: gif
- Widget: segmented
- UI location: VideoOptions.svelte
- ffmpeg: `palettegen=max_colors=<value>` filter.

### gif_dither
- Type: `String`
- Enum: `"none" | "bayer" | "floyd"`
- Default: `"floyd"`
- Applies to formats: gif
- Widget: segmented
- UI location: VideoOptions.svelte
- ffmpeg: `paletteuse=dither=<none | bayer | floyd_steinberg>`.

---

### Image — `imageOptions`

### jpeg_chroma
- Type: `String`
- Enum: `"420" | "422" | "444"`
- Default: `"420"`
- Applies to formats: jpeg
- Widget: segmented
- UI location: ImageOptions.svelte
- ImageMagick: `-sampling-factor 4:2:0 | 4:2:2 | 4:4:4`.

### jpeg_progressive
- Type: `bool`
- Default: `false`
- Applies to formats: jpeg
- Widget: checkbox
- UI location: ImageOptions.svelte
- ImageMagick: `-interlace Plane` when true.

### png_compression
- Type: `u32`
- Range: `0..=9`
- Default: `6`
- Applies to formats: png
- Widget: slider
- UI location: ImageOptions.svelte
- ImageMagick: `-define png:compression-level=<value>`.

### png_color_mode
- Type: `String`
- Enum: `"rgb" | "rgba" | "gray" | "graya" | "palette"`
- Default: `"rgba"`
- Applies to formats: png
- Widget: vertical segmented
- UI location: ImageOptions.svelte
- ImageMagick: `-define png:color-type=2|6|0|4|3` respectively.

### png_interlaced
- Type: `bool`
- Default: `false`
- Applies to formats: png
- Widget: checkbox
- UI location: ImageOptions.svelte
- ImageMagick: `-interlace Plane` when true.

### tiff_compression
- Type: `String`
- Enum: `"none" | "lzw" | "deflate" | "packbits"`
- Default: `"lzw"`
- Applies to formats: tiff
- Widget: segmented
- UI location: ImageOptions.svelte
- ImageMagick: `-compress None | LZW | Zip | RLE`.

### tiff_bit_depth
- Type: `u32`
- Enum: `8 | 16 | 32`
- Default: `8`
- Applies to formats: tiff
- Widget: segmented
- UI location: ImageOptions.svelte
- ImageMagick: `-depth <value>` (32 → `-define quantum:format=floating-point`).

### tiff_color_mode
- Type: `String`
- Enum: `"rgb" | "cmyk" | "gray"`
- Default: `"rgb"`
- Applies to formats: tiff
- Widget: segmented
- UI location: ImageOptions.svelte
- ImageMagick: `-colorspace RGB | CMYK | Gray`.

### webp_lossless
- Type: `bool`
- Default: `false`
- Applies to formats: webp
- Widget: checkbox
- UI location: ImageOptions.svelte
- ImageMagick: `-define webp:lossless=true`.

### webp_method
- Type: `u32`
- Range: `0..=6`
- Default: `4`
- Applies to formats: webp
- Widget: slider
- UI location: ImageOptions.svelte
- ImageMagick: `-define webp:method=<value>`.

### avif_speed
- Type: `u32`
- Range: `0..=10`
- Default: `6`
- Applies to formats: avif
- Widget: slider
- UI location: ImageOptions.svelte
- ImageMagick: `-define heic:speed=<value>` (libheif) or `-define avif:speed=<value>`.

### avif_chroma
- Type: `String`
- Enum: `"420" | "422" | "444"`
- Default: `"420"`
- Applies to formats: avif
- Widget: segmented
- UI location: ImageOptions.svelte
- ImageMagick: `-sampling-factor 4:2:0 | 4:2:2 | 4:4:4`.

### bmp_bit_depth
- Type: `u32`
- Enum: `8 | 16 | 24 | 32`
- Default: `24`
- Applies to formats: bmp
- Widget: segmented
- UI location: ImageOptions.svelte
- ImageMagick: `-depth <value>` + appropriate colorspace.

---

### Archive — `archiveOptions`

### archive_compression
- Type: `u32`
- Range: `0..=9`
- Default: `5`
- Applies to formats: zip, gz, 7z (hidden for plain tar)
- Widget: slider
- UI location: ArchiveOptions.svelte
- Tool mapping: zip → `-<level>` flag; gzip → `-<level>`; 7z → `-mx=<level>`.

---

## 3. Summary

- **Formats audited**: 35 non-`todo` formats across 6 categories (12 audio, 6 video, 6 image, 5 data, 4 document, 4 archive — doc group audited as intentionally sparse).
- **New option fields added**: 34 (13 audio, 12 video, 13 image / some overlap via shared `channels` + `bit_depth`, 1 archive).
- **New component**: `src/lib/ArchiveOptions.svelte`.
- **Components edited**: `App.svelte` (state + archive wiring), `AudioOptions.svelte`, `VideoOptions.svelte`, `ImageOptions.svelte`.
- **Build status**: `npm run build` passes clean.

All controls are scoped with `{#if options.output_format === '<fmt>'}` guards so each format only shows the knobs that apply to it. Defaults are picked so that leaving every slider untouched produces standard-quality output matching current behavior.
