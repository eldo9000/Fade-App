// Pure utility functions extracted from App.svelte for testability.

const IMAGE_EXTS = ['jpg','jpeg','png','webp','tiff','tif','bmp','gif','avif','heic','heif','psd','svg','ico','raw','cr2','nef','arw','dng'];
const VIDEO_EXTS = ['mp4','mkv','webm','avi','mov','m4v','flv','wmv','ts','mpg','mpeg','3gp','ogv'];
const AUDIO_EXTS = ['mp3','wav','flac','ogg','aac','opus','m4a','wma','aiff'];
// `ipynb` lives here for UI grouping; the backend routes it to the
// notebook pipeline by input extension regardless of output format.
const DATA_EXTS  = ['csv','json','xml','yaml','yml','toml','tsv','ndjson','jsonl','ipynb'];
const DOC_EXTS   = ['md','markdown','html','htm','txt'];
const ARCHIVE_EXTS = ['zip','7z','tar','gz','bz2','xz','tgz','rar','iso','dmg','cbz','cbr'];
const MODEL_EXTS = ['obj','stl','ply','gltf','glb','dae','fbx','3ds','x3d'];
// Timeline / EDL formats — routed through OpenTimelineIO's otioconvert CLI.
// `.xml` (Premiere XML) is omitted because it's ambiguous with data XML.
const TIMELINE_EXTS = ['edl','fcpxml','otio','aaf'];
const FONT_EXTS = ['ttf','otf','woff','woff2'];
// Subtitle formats routed via ffmpeg. sbv/ttml remain flagged todo
// in the UI because ffmpeg can't read them (sbv) or only write them (ttml).
const SUBTITLE_EXTS = ['srt','vtt','ass','ssa'];
const EBOOK_EXTS = ['epub','mobi','azw3','fb2','lit'];
const EMAIL_EXTS = ['eml','mbox'];

const PRESET_RESOLUTIONS = ['original', '1920x1080', '1280x720', '854x480'];

/** Classify a lowercase file extension into 'image' | 'video' | 'audio' | 'data' | 'document' | 'archive' | 'model' | 'timeline' | 'font' | 'subtitle' | 'ebook' | 'email' | 'unknown'. */
export function mediaTypeFor(ext) {
  if (IMAGE_EXTS.includes(ext)) return 'image';
  if (VIDEO_EXTS.includes(ext)) return 'video';
  if (AUDIO_EXTS.includes(ext)) return 'audio';
  if (DATA_EXTS.includes(ext)) return 'data';
  if (DOC_EXTS.includes(ext)) return 'document';
  if (ARCHIVE_EXTS.includes(ext)) return 'archive';
  if (MODEL_EXTS.includes(ext)) return 'model';
  if (TIMELINE_EXTS.includes(ext)) return 'timeline';
  if (FONT_EXTS.includes(ext)) return 'font';
  if (SUBTITLE_EXTS.includes(ext)) return 'subtitle';
  if (EBOOK_EXTS.includes(ext)) return 'ebook';
  if (EMAIL_EXTS.includes(ext)) return 'email';
  return 'unknown';
}

/**
 * Validate conversion options before starting.
 * Returns an object whose keys are error identifiers and values are messages.
 * An empty object means no errors.
 */
export function validateOptions(videoOptions, audioOptions) {
  const errors = {};

  if (videoOptions.trim_start != null && videoOptions.trim_end != null) {
    if (videoOptions.trim_end <= videoOptions.trim_start) {
      errors.video_trim = 'End must be after start';
    }
  }

  if (audioOptions.trim_start != null && audioOptions.trim_end != null) {
    if (audioOptions.trim_end <= audioOptions.trim_start) {
      errors.audio_trim = 'End must be after start';
    }
  }

  if (videoOptions.resolution && !PRESET_RESOLUTIONS.includes(videoOptions.resolution)) {
    if (!/^\d+x\d+$/.test(videoOptions.resolution)) {
      errors.resolution = 'Resolution must be WxH (e.g. 1920x1080)';
    }
  }

  return errors;
}

/** Format bytes into a human-readable string. */
export function formatBytes(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

/** Format seconds as M:SS.s */
export function formatDuration(secs) {
  if (secs == null) return '—';
  const m = Math.floor(secs / 60);
  const s = (secs % 60).toFixed(1);
  return `${m}:${s.padStart(4, '0')}`;
}
