<script>
  import { invoke } from '@tauri-apps/api/core';
  import ChromaKeyPanel from './ChromaKeyPanel.svelte';
  import AnalysisTools from './AnalysisTools.svelte';

  // ── Props API ──────────────────────────────────────────────────────────────
  let {
    // Bindable outputs — mutations propagate up
    selectedItem    = $bindable(null),
    queue           = $bindable([]),
    replaceAudioPath = $bindable(null),
    // Plain read-only props
    selectedOperation = null,
    videoOptions    = null,
    audioOptions    = null,
    outputDir       = null,
    outputSeparator = '_',
    setStatus       = null,
    pickAuxFile     = null,
    onBack          = null,
    // Bindable tab state — App.svelte can pre-select the subtitling tab
    subtitlingTab   = $bindable('embed'),
  } = $props();

  // ── OPERATIONS label map (mirrors App.svelte OPERATIONS) ─────────────────
  const OPERATIONS = [
    { id: 'cut-noenc',      label: 'Cut/Extract' },
    { id: 'replace-audio',  label: 'Replace Audio' },
    { id: 'rewrap',         label: 'Rewrap' },
    { id: 'conform',        label: 'Conform' },
    { id: 'merge',          label: 'Merge' },
    { id: 'extract',        label: 'Extract' },
    { id: 'subtitling',     label: 'Subtitling' },
    { id: 'video-inserts',  label: 'Video Inserts' },
    { id: 'silence-remove', label: 'Silence Remover' },
    { id: 'remove-audio',   label: 'Remove Audio' },
    { id: 'remove-video',   label: 'Remove Video' },
    { id: 'metadata-strip', label: 'Strip Metadata' },
    { id: 'loop',           label: 'Loop' },
    { id: 'rotate-flip',    label: 'Rotate / Flip' },
    { id: 'speed',          label: 'Speed / Reverse' },
    { id: 'fade',           label: 'Crop / Fade' },
    { id: 'deinterlace',    label: 'Deinterlace' },
    { id: 'denoise',        label: 'Denoise' },
    { id: 'thumbnail',      label: 'Thumbnail' },
    { id: 'contact-sheet',  label: 'Contact Sheet' },
    { id: 'frame-export',   label: 'Frame Export' },
    { id: 'watermark',      label: 'Watermark' },
    { id: 'channel-tools',  label: 'Channel Tools' },
    { id: 'pad-silence',    label: 'Pad Silence' },
    { id: 'chroma-ffmpeg',  label: 'Chroma Key (FFmpeg)' },
    { id: 'loudness',       label: 'Loudness & True Peak' },
    { id: 'audio-norm',     label: 'Audio Normalize' },
    { id: 'cut-detect',     label: 'Cut Detection' },
    { id: 'black-detect',   label: 'Black Detection' },
    { id: 'vmaf',           label: 'VMAF' },
    { id: 'framemd5',       label: 'FrameMD5' },
  ];

  // ── State ─────────────────────────────────────────────────────────────────

  let cutMode = $state('cut');  // 'cut' | 'extract'
  let op = $derived(OPERATIONS.find(o => o.id === selectedOperation));

  // Replace audio
  let replaceAudioOffsetMs    = $state(0);
  let replaceAudioFitLength   = $state(false);
  let replaceAudioAutoSync    = $state(false);

  // Rewrap
  let rewrapFormat = $state('mp4');

  // Extract
  let extractMode = $state('video');

  // Merge
  let mergeSelection = $state([]);

  // Conform
  let conformFps        = $state('23.976');
  let conformResolution = $state('source');
  let conformPixFmt     = $state('yuv420p');
  let conformFpsAlgo    = $state('drop');
  let conformScaleAlgo  = $state('lanczos');
  let conformDither     = $state(true);

  // Silence remover
  let silenceThresholdDb = $state(-30);
  let silenceMinDurS     = $state(0.5);
  let silencePadMs       = $state(100);

  // Metadata strip
  let metadataStripMode  = $state('all');
  let metadataStripTitle = $state('');

  // Loop
  let loopCount = $state(2);

  // Rotate/flip
  let rotateFlipMode = $state('cw90');

  // Speed
  let speedPreset = $state('1');
  let speedCustom = $state(1.0);

  // Fade
  let fadeInS  = $state(0.5);
  let fadeOutS = $state(0.5);

  // Deinterlace
  let deinterlaceMode = $state('yadif');

  // Denoise
  let denoisePreset = $state('medium');

  // Thumbnail
  let thumbnailTime   = $state('00:00:01');
  let thumbnailFormat = $state('jpeg');

  // Contact sheet
  let contactGridCols = $state(4);
  let contactGridRows = $state(6);
  let contactFrames   = $state(24);

  // Frame export
  let frameExportMode     = $state('fps');
  let frameExportFps      = $state(1);
  let frameExportInterval = $state(5);
  let frameExportFormat   = $state('jpeg');

  // Watermark
  let watermarkPath    = $state(null);
  let watermarkCorner  = $state('br');
  let watermarkOpacity = $state(0.8);
  let watermarkScale   = $state(15);

  // Channel tools
  let channelToolsMode = $state('stereo_to_mono');

  // Pad silence
  let padSilenceHead = $state(0);
  let padSilenceTail = $state(0);

  // Subtitling · analyze tab
  let subLintCpsMax       = $state(21);
  let subLintMinDurMs     = $state(1000);
  let subLintMaxDurMs     = $state(7000);
  let subLintLineMaxChars = $state(42);
  let subLintMaxLines     = $state(2);
  let subDiffReferencePath = $state(null);
  let subLintResults  = $state(null);
  let subDiffResults  = $state(null);
  let subProbeResults = $state(null);

  // ── Helpers ───────────────────────────────────────────────────────────────

  // Mirrors src-tauri/src/lib.rs::build_output_path — keep in sync.
  function expectedOutputPath(item, newExt, suffix, outDir, sep = '_') {
    const lastSlash = item.path.lastIndexOf('/');
    const parentDir = lastSlash >= 0 ? item.path.slice(0, lastSlash) : '.';
    const dir = outDir ?? parentDir;
    const stem = item.ext ? item.name.slice(0, -(item.ext.length + 1)) : item.name;
    return suffix
      ? `${dir}/${stem}${sep}${suffix}.${newExt}`
      : `${dir}/${stem}.${newExt}`;
  }

  // ── Generic op runner ─────────────────────────────────────────────────────
  async function _runOp({ payload, suffix, outExt, label, requireVideo = false }) {
    if (!selectedItem) { setStatus?.('Select a file first', 'error'); return; }
    if (selectedItem.status === 'converting') return;
    if (requireVideo && selectedItem.mediaType !== 'video') {
      setStatus?.(`${label}: video file required`, 'error');
      return;
    }
    const ext = outExt || selectedItem.ext || (selectedItem.mediaType === 'video' ? 'mp4' : 'wav');
    const outPath = expectedOutputPath(selectedItem, ext, suffix, outputDir, outputSeparator);
    selectedItem.status = 'converting';
    selectedItem.percent = 0;
    selectedItem.error = null;
    try {
      await invoke('run_operation', {
        jobId: selectedItem.id,
        operation: { ...payload, input_path: selectedItem.path, output_path: outPath },
      });
    } catch (err) {
      selectedItem.status = 'error';
      selectedItem.error = String(err);
      setStatus?.(`${label} failed: ${err}`, 'error');
    }
  }

  // ── Operation functions ───────────────────────────────────────────────────

  async function runRewrap() {
    if (!selectedItem) { setStatus?.('Select a file first', 'error'); return; }
    if (selectedItem.status === 'converting') return;
    const outPath = expectedOutputPath(selectedItem, rewrapFormat, 'rewrap', outputDir, outputSeparator);
    selectedItem.status = 'converting';
    selectedItem.percent = 0;
    selectedItem.error = null;
    try {
      await invoke('run_operation', {
        jobId: selectedItem.id,
        operation: { type: 'rewrap', input_path: selectedItem.path, output_path: outPath },
      });
    } catch (err) {
      selectedItem.status = 'error';
      selectedItem.error = String(err);
      setStatus?.(`Rewrap failed: ${err}`, 'error');
    }
  }

  async function runCut() {
    if (!selectedItem) { setStatus?.('Select a file first', 'error'); return; }
    if (selectedItem.status === 'converting') return;
    const opts = selectedItem.mediaType === 'video' ? videoOptions : audioOptions;
    const outExt = selectedItem.ext || (selectedItem.mediaType === 'video' ? 'mp4' : 'wav');
    const outPath = expectedOutputPath(selectedItem, outExt, 'cut', outputDir, outputSeparator);
    selectedItem.status = 'converting';
    selectedItem.percent = 0;
    selectedItem.error = null;
    try {
      await invoke('run_operation', {
        jobId: selectedItem.id,
        operation: {
          type: 'cut',
          input_path: selectedItem.path,
          start_secs: opts?.trim_start ?? null,
          end_secs: opts?.trim_end ?? null,
          output_path: outPath,
        },
      });
    } catch (err) {
      selectedItem.status = 'error';
      selectedItem.error = String(err);
      setStatus?.(`Cut failed: ${err}`, 'error');
    }
  }

  async function runReplaceAudio() {
    if (!selectedItem || selectedItem.mediaType !== 'video') {
      setStatus?.('Select a video first', 'error'); return;
    }
    if (!replaceAudioPath) { setStatus?.('Pick a replacement audio file', 'error'); return; }
    if (selectedItem.status === 'converting') return;
    const outPath = expectedOutputPath(selectedItem, selectedItem.ext || 'mp4', 'replaced', outputDir, outputSeparator);
    selectedItem.status = 'converting';
    selectedItem.percent = 0;
    selectedItem.error = null;
    try {
      await invoke('run_operation', {
        jobId: selectedItem.id,
        operation: {
          type: 'replace_audio',
          video_path: selectedItem.path,
          audio_path: replaceAudioPath,
          output_path: outPath,
        },
      });
    } catch (err) {
      selectedItem.status = 'error';
      selectedItem.error = String(err);
      setStatus?.(`Replace audio failed: ${err}`, 'error');
    }
  }

  async function runMerge() {
    if (mergeSelection.length < 2) {
      setStatus?.('Add at least 2 files to merge', 'error'); return;
    }
    const items = mergeSelection
      .map(id => queue.find(q => q.id === id))
      .filter(Boolean);
    if (items.length < 2) { setStatus?.('Merge list has missing items', 'error'); return; }

    const first = items[0];
    const outExt = first.ext || 'mp4';
    const outPath = expectedOutputPath(first, outExt, 'merged', outputDir, outputSeparator);

    first.status = 'converting';
    first.percent = 0;
    first.error = null;
    try {
      await invoke('run_operation', {
        jobId: first.id,
        operation: {
          type: 'merge',
          input_paths: items.map(i => i.path),
          output_path: outPath,
        },
      });
    } catch (err) {
      first.status = 'error';
      first.error = String(err);
      setStatus?.(`Merge failed: ${err}`, 'error');
    }
  }

  async function runExtract() {
    if (!selectedItem) { setStatus?.('Select a file first', 'error'); return; }
    if (selectedItem.status === 'converting') return;
    let streams = [];
    try {
      streams = await invoke('get_streams', { inputPath: selectedItem.path });
    } catch (err) {
      setStatus?.(`Extract: probe failed: ${err}`, 'error');
      return;
    }
    let targets = [];
    if (extractMode === 'video') {
      const v = streams.find(s => s.stream_type === 'video');
      if (v) targets = [v];
    } else if (extractMode === 'all') {
      targets = streams.filter(s => ['video', 'audio', 'subtitle'].includes(s.stream_type));
    } else {
      targets = streams.filter(s => s.stream_type === extractMode);
    }
    if (targets.length === 0) {
      setStatus?.(`No ${extractMode} streams found`, 'error'); return;
    }

    selectedItem.status = 'converting';
    selectedItem.percent = 0;
    selectedItem.error = null;

    for (const s of targets) {
      const ext = s.stream_type === 'video'
        ? (s.codec === 'h264' ? 'h264' : s.codec === 'hevc' ? 'hevc' : 'mkv')
        : s.stream_type === 'audio'
        ? (s.codec === 'aac' ? 'aac' : s.codec === 'mp3' ? 'mp3' : s.codec === 'opus' ? 'opus' : s.codec === 'flac' ? 'flac' : 'mka')
        : s.stream_type === 'subtitle'
        ? (s.codec === 'subrip' ? 'srt' : s.codec === 'ass' || s.codec === 'ssa' ? 'ass' : s.codec === 'webvtt' ? 'vtt' : 'mks')
        : 'bin';
      const suffix = targets.length > 1
        ? `extract_${s.stream_type}_${s.index}`
        : `extract_${s.stream_type}`;
      const outPath = expectedOutputPath(selectedItem, ext, suffix, outputDir, outputSeparator);
      const jobId = targets.length === 1 || s === targets[targets.length - 1]
        ? selectedItem.id
        : `${selectedItem.id}__extract_${s.index}`;
      try {
        await invoke('run_operation', {
          jobId,
          operation: {
            type: 'extract',
            input_path: selectedItem.path,
            stream_index: s.index,
            stream_type: s.stream_type,
            output_path: outPath,
          },
        });
      } catch (err) {
        setStatus?.(`Extract stream ${s.index} failed: ${err}`, 'error');
      }
    }
  }

  async function runConform() {
    if (!selectedItem || selectedItem.mediaType !== 'video') {
      setStatus?.('Select a video first', 'error');
      return;
    }
    if (selectedItem.status === 'converting') return;

    const outPath = expectedOutputPath(selectedItem, 'mp4', 'conformed', outputDir, outputSeparator);
    selectedItem.status = 'converting';
    selectedItem.percent = 0;
    selectedItem.error = null;

    try {
      await invoke('run_operation', {
        jobId: selectedItem.id,
        operation: {
          type: 'conform',
          input_path: selectedItem.path,
          output_path: outPath,
          fps: conformFps === 'source' ? null : conformFps,
          resolution: conformResolution === 'source' ? null : conformResolution,
          pix_fmt: conformPixFmt === 'source' ? null : conformPixFmt,
          fps_algo: conformFpsAlgo,
          scale_algo: conformScaleAlgo,
          dither: conformDither,
        },
      });
    } catch (err) {
      selectedItem.status = 'error';
      selectedItem.error = String(err);
      setStatus?.(`Conform failed: ${err}`, 'error');
    }
  }

  async function runSilenceRemove() {
    if (!selectedItem) { setStatus?.('Select a file first', 'error'); return; }
    if (selectedItem.status === 'converting') return;
    const outExt = selectedItem.ext || (selectedItem.mediaType === 'video' ? 'mp4' : 'wav');
    const outPath = expectedOutputPath(selectedItem, outExt, 'unsilenced', outputDir, outputSeparator);
    selectedItem.status = 'converting';
    selectedItem.percent = 0;
    selectedItem.error = null;
    try {
      await invoke('run_operation', {
        jobId: selectedItem.id,
        operation: {
          type: 'silence_remove',
          input_path: selectedItem.path,
          output_path: outPath,
          threshold_db: Number(silenceThresholdDb),
          min_silence_s: Number(silenceMinDurS),
          pad_ms: Number(silencePadMs),
        },
      });
    } catch (err) {
      selectedItem.status = 'error';
      selectedItem.error = String(err);
      setStatus?.(`Silence remove failed: ${err}`, 'error');
    }
  }

  async function runRemoveAudio() {
    const outExt = selectedItem?.ext || 'mp4';
    return _runOp({ payload: { type: 'remove_audio' }, suffix: 'noaudio', outExt, label: 'Remove audio' });
  }

  async function runRemoveVideo() {
    return _runOp({ payload: { type: 'remove_video' }, suffix: 'audio-only', outExt: 'mka', label: 'Remove video' });
  }

  async function runMetadataStrip() {
    return _runOp({
      payload: {
        type: 'metadata_strip',
        mode: metadataStripMode,
        title_value: metadataStripMode === 'title' ? String(metadataStripTitle || '') : null,
      },
      suffix: 'stripped',
      label: 'Metadata strip',
    });
  }

  async function runLoop() {
    const n = Math.max(2, Math.min(50, Number(loopCount) || 2));
    return _runOp({
      payload: { type: 'loop', count: n },
      suffix: `x${n}`,
      label: 'Loop',
    });
  }

  async function runRotateFlip() {
    return _runOp({
      payload: { type: 'rotate_flip', mode: rotateFlipMode },
      suffix: 'rotated',
      outExt: 'mp4',
      label: 'Rotate/Flip',
      requireVideo: true,
    });
  }

  async function runReverse() {
    return _runOp({
      payload: { type: 'reverse' },
      suffix: 'reversed',
      outExt: 'mp4',
      label: 'Reverse',
    });
  }

  async function runSpeed() {
    const rate = speedPreset === 'custom'
      ? Math.max(0.1, Math.min(10.0, Number(speedCustom) || 1.0))
      : Number(speedPreset);
    return _runOp({
      payload: { type: 'speed', rate },
      suffix: `${rate}x`,
      outExt: 'mp4',
      label: 'Speed',
    });
  }

  async function runFade() {
    const fi = Math.max(0, Math.min(10, Number(fadeInS) || 0));
    const fo = Math.max(0, Math.min(10, Number(fadeOutS) || 0));
    if (fi === 0 && fo === 0) { setStatus?.('Set at least one fade value', 'error'); return; }
    return _runOp({
      payload: { type: 'fade', fade_in: fi, fade_out: fo },
      suffix: 'faded',
      outExt: 'mp4',
      label: 'Fade',
    });
  }

  async function runDeinterlace() {
    return _runOp({
      payload: { type: 'deinterlace', mode: deinterlaceMode },
      suffix: 'deint',
      outExt: 'mp4',
      label: 'Deinterlace',
      requireVideo: true,
    });
  }

  async function runDenoise() {
    return _runOp({
      payload: { type: 'denoise', preset: denoisePreset },
      suffix: 'denoised',
      outExt: 'mp4',
      label: 'Denoise',
      requireVideo: true,
    });
  }

  async function runThumbnail() {
    const ext = thumbnailFormat === 'jpeg' ? 'jpg' : thumbnailFormat;
    const base = selectedItem?.path || '';
    const lastSlash = base.lastIndexOf('/');
    const parentDir = lastSlash >= 0 ? base.slice(0, lastSlash) : '.';
    const stem = selectedItem?.ext
      ? selectedItem.name.slice(0, -(selectedItem.ext.length + 1))
      : (selectedItem?.name || 'frame');
    const dir = outputDir ?? parentDir;
    const outPath = `${dir}/${stem}${outputSeparator}thumb.${ext}`;
    if (!selectedItem) { setStatus?.('Select a file first', 'error'); return; }
    selectedItem.status = 'converting'; selectedItem.percent = 0; selectedItem.error = null;
    try {
      await invoke('run_operation', {
        jobId: selectedItem.id,
        operation: {
          type: 'thumbnail',
          input_path: selectedItem.path,
          output_path: outPath,
          time_spec: String(thumbnailTime || '0'),
          format: thumbnailFormat,
        },
      });
    } catch (err) {
      selectedItem.status = 'error'; selectedItem.error = String(err);
      setStatus?.(`Thumbnail failed: ${err}`, 'error');
    }
  }

  async function runContactSheet() {
    const c = Math.max(1, Math.min(20, Number(contactGridCols) || 4));
    const r = Math.max(1, Math.min(20, Number(contactGridRows) || 6));
    const n = Math.max(c * r, Math.min(500, Number(contactFrames) || c * r));
    return _runOp({
      payload: { type: 'contact_sheet', cols: c, rows: r, frames: n },
      suffix: 'sheet',
      outExt: 'png',
      label: 'Contact sheet',
      requireVideo: true,
    });
  }

  async function runFrameExport() {
    if (!selectedItem) { setStatus?.('Select a file first', 'error'); return; }
    const base = selectedItem.path;
    const lastSlash = base.lastIndexOf('/');
    const parentDir = lastSlash >= 0 ? base.slice(0, lastSlash) : '.';
    const stem = selectedItem.ext
      ? selectedItem.name.slice(0, -(selectedItem.ext.length + 1))
      : selectedItem.name;
    const dir = outputDir ?? parentDir;
    const outDir = `${dir}/${stem}_frames`;
    const value = frameExportMode === 'fps'
      ? Math.max(0.01, Number(frameExportFps) || 1)
      : Math.max(0.01, Number(frameExportInterval) || 5);
    selectedItem.status = 'converting'; selectedItem.percent = 0; selectedItem.error = null;
    try {
      await invoke('run_operation', {
        jobId: selectedItem.id,
        operation: {
          type: 'frame_export',
          input_path: selectedItem.path,
          output_dir: outDir,
          mode: frameExportMode,
          value,
          format: frameExportFormat,
        },
      });
    } catch (err) {
      selectedItem.status = 'error'; selectedItem.error = String(err);
      setStatus?.(`Frame export failed: ${err}`, 'error');
    }
  }

  async function runWatermark() {
    if (!watermarkPath) { setStatus?.('Pick a watermark PNG first', 'error'); return; }
    return _runOp({
      payload: {
        type: 'watermark',
        watermark_path: watermarkPath,
        corner: watermarkCorner,
        opacity: Number(watermarkOpacity),
        scale_pct: Number(watermarkScale),
      },
      suffix: 'watermarked',
      outExt: 'mp4',
      label: 'Watermark',
      requireVideo: true,
    });
  }

  async function runChannelTools() {
    return _runOp({
      payload: { type: 'channel_tools', mode: channelToolsMode },
      suffix: 'ch-' + channelToolsMode,
      label: 'Channel tools',
    });
  }

  async function runPadSilence() {
    const h = Math.max(0, Math.min(60, Number(padSilenceHead) || 0));
    const t = Math.max(0, Math.min(60, Number(padSilenceTail) || 0));
    if (h === 0 && t === 0) { setStatus?.('Set at least one pad value', 'error'); return; }
    return _runOp({
      payload: { type: 'pad_silence', head_s: h, tail_s: t },
      suffix: 'padded',
      label: 'Pad silence',
    });
  }

  async function runSubLint() {
    if (!selectedItem) { setStatus?.('Select a subtitle file first', 'error'); return; }
    subLintResults = null;
    try {
      subLintResults = await invoke('lint_subtitle', {
        inputPath: selectedItem.path,
        thresholds: {
          cps_max: Number(subLintCpsMax),
          min_dur_ms: Number(subLintMinDurMs),
          max_dur_ms: Number(subLintMaxDurMs),
          line_max_chars: Number(subLintLineMaxChars),
          max_lines: Number(subLintMaxLines),
        },
      });
      setStatus?.(`${subLintResults.length} lint issues`, 'success');
    } catch (err) {
      setStatus?.(`Subtitle lint failed: ${err}`, 'error');
    }
  }

  async function runSubDiff() {
    if (!selectedItem || !subDiffReferencePath) return;
    subDiffResults = null;
    try {
      subDiffResults = await invoke('diff_subtitle', {
        aPath: selectedItem.path,
        bPath: subDiffReferencePath,
      });
      setStatus?.('Subtitle diff complete', 'success');
    } catch (err) {
      setStatus?.(`Subtitle diff failed: ${err}`, 'error');
    }
  }

  async function runSubProbe() {
    if (!selectedItem) { setStatus?.('Select a video first', 'error'); return; }
    subProbeResults = null;
    try {
      subProbeResults = await invoke('probe_subtitles', { inputPath: selectedItem.path });
      setStatus?.(`${subProbeResults.length} subtitle streams`, 'success');
    } catch (err) {
      setStatus?.(`Subtitle probe failed: ${err}`, 'error');
    }
  }
</script>

<div class="absolute inset-0 flex flex-col items-center justify-end p-8 overflow-y-auto gap-6 {selectedItem?.mediaType === 'video' ? 'pb-[340px]' : ''}">
  <div class="flex items-center gap-3 justify-center w-[560px]">
    <button
      onclick={onBack}
      class="px-3 py-1.5 rounded text-[11px] font-medium border border-[var(--border)]
             text-white hover:border-[var(--accent)] hover:bg-white/5 transition-colors shrink-0"
    >← Back</button>
    <h2 class="text-[20px] font-semibold text-white/85">{op?.label}</h2>
  </div>

  {#if selectedItem?.mediaType !== 'video'}
    <div class="w-[560px] aspect-video rounded-lg border border-dashed border-white/10
                flex items-center justify-center text-[10px] text-white/25 bg-black/30 shrink-0">
      Video preview
    </div>
  {/if}

  <div class="rounded-lg border border-white/10 bg-white/[0.04] px-5 py-4 w-[560px] shrink-0">
    <p class="text-[11px] uppercase tracking-wider text-white/35 font-semibold mb-2">How it works</p>
    <p class="text-[13px] text-white/60 leading-relaxed">
      {#if selectedOperation === 'cut-noenc'}
        {#if cutMode === 'cut'}
          <strong class="text-white/80">Cut</strong> — keep the range between the trim handles and discard the rest. Stream copy, no re-encoding. Cuts snap to the nearest keyframe.
        {:else}
          <strong class="text-white/80">Extract</strong> — remove the range between the trim handles and glue the two halves back together. Requires re-encoding because the join point would otherwise break the GOP.
        {/if}
        <br/><br/><span class="text-white/35">Drag the trim handles on the timeline to set the range. Flip between Cut / Extract with the toggle below.</span>
      {:else if selectedOperation === 'replace-audio'}
        Swap the audio track in a video with a new audio file. Video is copied untouched; only the audio is remuxed. If the new audio codec is incompatible with the container, it's transcoded to AAC automatically.
        <br/><br/><span class="text-white/35">Select the video in the queue. Drag a new audio file as the replacement source.</span>
      {:else if selectedOperation === 'rewrap'}
        Change the container format without re-encoding any streams — fast and lossless. Example: MKV → MP4. Codec compatibility with the target container is checked first; incompatible streams are flagged before the job starts.
        <br/><br/><span class="text-white/35">Select a file from the queue and choose an output container.</span>
      {:else if selectedOperation === 'conform'}
        Match a video to a reference specification: frame rate, resolution, and pixel format. Always requires re-encoding. Use fps=24000/1001 for broadcast 23.976 or fps=30000/1001 for 29.97.
        <br/><br/><span class="text-white/35">Set target fps, resolution, and pixel format in the controls below.</span>
      {:else if selectedOperation === 'merge'}
        Concatenate multiple video files into one in order. Files with matching codec, resolution, fps, and pixel format merge without re-encoding via the concat demuxer. Mismatched files are re-encoded to H.264/AAC automatically.
        <br/><br/><span class="text-white/35">Add files to the queue in order — they will be joined top to bottom.</span>
      {:else if selectedOperation === 'extract'}
        Pull individual streams out of a container: video-only, a specific audio track by index, or a subtitle track. Each stream is written to its own output file with no re-encoding.
        <br/><br/><span class="text-white/35">Select a file, inspect its streams in the panel, and choose what to extract.</span>
      {:else if selectedOperation === 'subtitling'}
        Two modes: burn-in (hard subs — subtitles are rendered into the video pixels, always re-encodes) or embed (soft subs — subtitle file is added as a selectable track, video is not re-encoded). SRT, ASS, VTT, and SSA formats supported.
        <br/><br/><span class="text-white/35">Drag a subtitle file alongside the video. Choose burn-in or embed mode.</span>
      {:else if selectedOperation === 'video-inserts'}
        Insert a video clip at a specific timecode in the main video. The source is split at the insert point, the clip is placed between the two halves, and everything is rejoined. Stream copy is used when all clips share the same codec and specs.
        <br/><br/><span class="text-white/35">Mark the insert point on the timeline, then drag the insert clip into position.</span>
      {:else if selectedOperation === 'silence-remove'}
        Detect silent gaps and cut them out. Uses ffmpeg <strong class="text-white/80">silenceremove</strong>. Threshold sets the dBFS floor; anything quieter than that for at least the minimum duration is removed. A padding value keeps a bit of silence around each speech region so onsets don't clip.
        <br/><br/><span class="text-white/35">Tune threshold / min duration / padding and Run. Video stream is re-cut to follow the audio.</span>
      {:else if selectedOperation === 'remove-audio'}
        Strip all audio tracks from a container while stream-copying the video. Uses <strong class="text-white/80">-map 0 -map -0:a -c copy</strong>. Output has the same video codec, container, and metadata — no re-encode.
        <br/><br/><span class="text-white/35">Click Run. Subtitles / chapters / attachments pass through unchanged.</span>
      {:else if selectedOperation === 'remove-video'}
        Drop the video track and keep audio stream-copied. Output container is an audio-only Matroska (.mka) because it accepts any audio codec losslessly.
        <br/><br/><span class="text-white/35">Click Run. For a specific audio container (mp3/aac/flac), use Extract instead.</span>
      {:else if selectedOperation === 'metadata-strip'}
        Remove container-level metadata and chapters via <strong class="text-white/80">-map_metadata -1</strong>. Streams are copied untouched. Choose whether to wipe every tag, or wipe everything and re-write only a new <code>title</code>.
        <br/><br/><span class="text-white/35">Note: this does not touch per-stream metadata inside codec bitstreams (e.g. embedded ID3 inside MP3).</span>
      {:else if selectedOperation === 'loop'}
        Repeat the clip <strong class="text-white/80">N</strong> times end-to-end via <code>-stream_loop</code>. Streams are copied — no re-encode — so output file size is roughly N × input.
        <br/><br/><span class="text-white/35">Allowed range: 2–50 playthroughs.</span>
      {:else if selectedOperation === 'rotate-flip'}
        Rotate 90°/180° or mirror horizontally/vertically. Uses <code>transpose</code>, <code>hflip</code>, or <code>vflip</code> filters and re-encodes to H.264 / AAC.
        <br/><br/><span class="text-white/35">Pick a direction and Run. 180° is two transpose=1 passes.</span>
      {:else if selectedOperation === 'speed'}
        Change playback rate with pitch-preserved audio, or play the clip backwards. Uses <code>setpts=PTS/R</code> for video and chained <code>atempo</code> for audio (atempo's 0.5–2.0 range is handled automatically). Reverse uses <code>reverse</code> + <code>areverse</code> and is <strong class="text-white/80">memory-heavy</strong>.
        <br/><br/><span class="text-white/35">Pick a preset or enter a custom rate (0.1–10×). Reverse runs independently; trim first for long clips.</span>
      {:else if selectedOperation === 'fade'}
        Crop uses the timeline's trim handles. Add a fade-in from black and/or a fade-out to black at the tail — <code>fade=t=in</code> / <code>fade=t=out</code> plus matching <code>afade</code>.
        <br/><br/><span class="text-white/35">Set the fade values; use the timeline handles for crop. Either fade defaults to 0.5s; set to 0 to skip.</span>
      {:else if selectedOperation === 'deinterlace'}
        Convert interlaced footage to progressive. <strong class="text-white/80">yadif</strong> is fast & safe; <strong class="text-white/80">yadif 2×</strong> doubles the framerate using both fields; <strong class="text-white/80">bwdif</strong> is higher-quality at ~2× the CPU cost.
        <br/><br/><span class="text-white/35">Use yadif for 1080i broadcast; bwdif when artifacts matter.</span>
      {:else if selectedOperation === 'denoise'}
        Temporal + spatial noise reduction via <code>hqdn3d</code>. Three presets (light/medium/strong) map to escalating luma/chroma spatial and temporal coefficients.
        <br/><br/><span class="text-white/35">Stronger = softer image. Watch for detail loss on faces and textures.</span>
      {:else if selectedOperation === 'thumbnail'}
        Seek to a timestamp and write a single frame as JPEG / PNG / WebP. Uses <code>-ss T -frames:v 1</code>.
        <br/><br/><span class="text-white/35">Time accepts <code>HH:MM:SS.ms</code> or plain seconds.</span>
      {:else if selectedOperation === 'contact-sheet'}
        Tile N frames into a single PNG grid. Uses <code>select</code> + <code>tile</code>. Handy for quickly previewing a long clip's content.
        <br/><br/><span class="text-white/35">Default grid: 4×6 (24 frames). Increase frames to oversample.</span>
      {:else if selectedOperation === 'frame-export'}
        Emit an image sequence to a sibling <code>&lt;name&gt;_frames/</code> folder. Choose a fixed frames-per-second rate or a fixed time interval between frames.
        <br/><br/><span class="text-white/35">Output pattern: <code>frame_000001.jpg</code> etc. Be mindful — this can produce thousands of files.</span>
      {:else if selectedOperation === 'watermark'}
        Overlay a PNG watermark at a chosen corner with opacity and size (% of video width). Audio is stream-copied; video re-encodes to H.264.
        <br/><br/><span class="text-white/35">Transparent PNGs work best. Use opacity to further tame a logo.</span>
      {:else if selectedOperation === 'channel-tools'}
        Channel manipulations via the <code>pan</code> filter. Downmix stereo to mono, swap L/R, mute a side, or fake stereo from a mono source.
        <br/><br/><span class="text-white/35">For true upmix/downmix with proper LFE handling, use a dedicated audio editor.</span>
      {:else if selectedOperation === 'pad-silence'}
        Prepend or append silence. Uses <code>adelay</code> for head padding and <code>apad=pad_dur</code> for tail padding.
        <br/><br/><span class="text-white/35">Each side: 0–60 seconds. Output is longer than the input by head + tail.</span>
      {:else if selectedOperation === 'chroma-ffmpeg'}
        Remove a solid background colour using an FFmpeg built-in filter. <strong class="text-white/80">chromakey</strong> (YUV) is the default for green/blue screens; <strong class="text-white/80">colorkey</strong> (RGB) gives hard cuts for flat mattes; <strong class="text-white/80">hsvkey</strong> handles uneven screen lighting. Optional <code>despill</code> removes coloured light bouncing onto the subject. Output is an alpha-capable container — ProRes 4444, VP9+alpha, PNG sequence, QtRLE, or FFV1.
        <br/><br/><span class="text-white/35">Pick a colour, tune similarity/blend, scrub to a frame and Preview. Trim handles limit the keyed segment.</span>
      {:else if selectedOperation === 'loudness'}
        Measure <strong class="text-white/80">EBU R128</strong> loudness: integrated LUFS (I), loudness range (LRA), and true-peak (dBTP). Read-only analysis — no file is written. True-peak uses 4× oversampling for accuracy and is slower.
        <br/><br/><span class="text-white/35">Pick a target preset (broadcast / streaming / Spotify) and Analyze. Results appear below.</span>
      {:else if selectedOperation === 'audio-norm'}
        Normalize audio loudness. <strong class="text-white/80">EBU R128</strong> (two-pass loudnorm, recommended) measures first then applies a matched gain curve; <strong class="text-white/80">Peak</strong> scales the max sample to a ceiling; <strong class="text-white/80">ReplayGain</strong> writes tags without touching samples.
        <br/><br/><span class="text-white/35">Choose mode, set targets, Run. Video streams are stream-copied.</span>
      {:else if selectedOperation === 'cut-detect'}
        Find shot/scene changes. <strong class="text-white/80">scdet</strong> (FFmpeg ≥4.4) scores each frame transition; <strong class="text-white/80">scene</strong> uses the classic select filter. Higher threshold = fewer false positives. Downscale first for a 10× speedup.
        <br/><br/><span class="text-white/35">Results list timestamps — click any entry to seek the timeline.</span>
      {:else if selectedOperation === 'black-detect'}
        Detect black intervals (fades, slates, leader). Emits start/end/duration for each run longer than the min-duration threshold. Pairs naturally with cut detection for fade-to-black transitions.
        <br/><br/><span class="text-white/35">Tune min duration and pixel/picture thresholds, then Analyze.</span>
      {:else if selectedOperation === 'vmaf'}
        Netflix <strong class="text-white/80">VMAF</strong> perceptual quality score comparing a distorted encode against a reference. Outputs a mean score 0–100. Reference and distorted must match in resolution and frame rate — the tool auto-scales both inputs to the model's native size.
        <br/><br/><span class="text-white/35">Drop the reference and distorted files, pick a model, Run. Subsample &gt;1 skips frames for speed.</span>
      {:else if selectedOperation === 'framemd5'}
        Emit an MD5 hash per decoded frame for bit-exactness verification. Hashes are over decoded pixels (or PCM samples) so they're codec-agnostic. Diff mode compares two files line-by-line to locate the first frame of divergence.
        <br/><br/><span class="text-white/35">Pick streams (video / audio / both). Add a second file to run a diff.</span>
      {/if}
    </p>
  </div>

  <div class="flex flex-col gap-3 w-[560px] shrink-0">
    <p class="text-[10px] uppercase tracking-wider text-white/30 font-semibold">Controls</p>
    <div class="flex flex-wrap gap-2">
      {#if selectedOperation === 'cut-noenc'}
        <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)]">
          <button
            onclick={() => cutMode = 'cut'}
            class="px-3 py-1.5 text-[12px] font-semibold transition-colors
                   {cutMode === 'cut' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}"
          >Cut</button>
          <div class="w-px h-6 bg-[var(--border)]"></div>
          <button
            onclick={() => cutMode = 'extract'}
            class="px-3 py-1.5 text-[12px] font-semibold transition-colors
                   {cutMode === 'extract' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}"
          >Extract</button>
        </div>
        <button
          onclick={() => cutMode === 'cut' ? runCut() : runExtract()}
          disabled={!selectedItem || selectedItem.status === 'converting'}
          class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
        >Run {cutMode === 'cut' ? 'Cut' : 'Extract'}</button>
      {:else if selectedOperation === 'replace-audio'}
        <div class="flex flex-wrap items-center gap-2 w-full">
          <button
            onclick={() => replaceAudioPath
              ? (replaceAudioPath = null)
              : pickAuxFile?.((p) => replaceAudioPath = p, 'audio/*,video/*')}
            class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors"
          >{replaceAudioPath ? 'Clear replacement' : 'Pick audio file…'}</button>
          <button class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">Keep original tracks</button>
          <button
            onclick={() => replaceAudioAutoSync = !replaceAudioAutoSync}
            title="Cross-correlate to find offset, pitch-preserved stretch to match length, and reuse the video's existing audio sample-rate & codec."
            class="px-3 py-1.5 rounded text-[12px] font-semibold border transition-colors
                   {replaceAudioAutoSync
                     ? 'bg-[var(--accent)] border-[var(--accent)] text-white'
                     : 'border-[var(--accent)] text-[var(--accent)] hover:bg-[color-mix(in_srgb,var(--accent)_12%,transparent)]'}"
          >✦ Auto-sync</button>
          <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)]">
            <span class="px-2 text-[10px] uppercase tracking-wider text-white/40 font-semibold">Stretch</span>
            <div class="w-px h-6 bg-[var(--border)]"></div>
            <button
              onclick={() => replaceAudioFitLength = false}
              class="px-3 py-1.5 text-[12px] font-semibold transition-colors
                     {!replaceAudioFitLength ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}"
            >Off</button>
            <div class="w-px h-6 bg-[var(--border)]"></div>
            <button
              onclick={() => replaceAudioFitLength = true}
              class="px-3 py-1.5 text-[12px] font-semibold transition-colors
                     {replaceAudioFitLength ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}"
            >Fit to length</button>
          </div>
        </div>
        <div class="flex items-center gap-3 w-full">
          <button
            onclick={runReplaceAudio}
            disabled={!selectedItem || selectedItem.mediaType !== 'video' || !replaceAudioPath || selectedItem.status === 'converting'}
            class="shrink-0 px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
          >Run Replace</button>
          <div class="flex items-center gap-2 flex-1 min-w-0">
            <span class="text-[10px] uppercase tracking-wider text-white/40 font-semibold shrink-0">Offset</span>
            <input
              type="range"
              min="-2000"
              max="2000"
              step="10"
              bind:value={replaceAudioOffsetMs}
              ondblclick={() => replaceAudioOffsetMs = 0}
              class="flex-1 min-w-0 accent-[var(--accent)]"
              title="Double-click to reset to 0"
            />
            <input
              type="number"
              step="10"
              bind:value={replaceAudioOffsetMs}
              class="w-16 shrink-0 bg-black/30 border border-[var(--border)] rounded px-1.5 py-0.5 text-[12px] text-white outline-none text-right font-mono tabular-nums focus:border-[var(--accent)]"
            />
            <span class="text-[10px] text-white/30 shrink-0">ms</span>
          </div>
        </div>
      {:else if selectedOperation === 'rewrap'}
        {#each ['mp4', 'mkv', 'mov', 'webm'] as fmt}
          <button
            onclick={() => rewrapFormat = fmt}
            class="px-3 py-1.5 rounded text-[12px] font-semibold border transition-colors
                   {rewrapFormat === fmt
                     ? 'bg-[var(--accent)] border-[var(--accent)] text-white'
                     : 'border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)]'}"
          >{fmt.toUpperCase()}</button>
        {/each}
        <button
          onclick={runRewrap}
          disabled={!selectedItem || selectedItem.status === 'converting'}
          class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
        >Run Rewrap</button>
      {:else if selectedOperation === 'conform'}
        <div class="flex flex-wrap items-center gap-2 w-full">
          <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
            <!-- svelte-ignore a11y_label_has_associated_control -->
            <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">FPS</label>
            <select bind:value={conformFps}
                    class="bg-transparent text-[12px] text-white outline-none font-mono tabular-nums">
              <option value="source">source</option>
              <option value="23.976">23.976</option>
              <option value="24">24</option>
              <option value="25">25</option>
              <option value="29.97">29.97</option>
              <option value="30">30</option>
              <option value="50">50</option>
              <option value="59.94">59.94</option>
              <option value="60">60</option>
            </select>
          </div>
          <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
            <!-- svelte-ignore a11y_label_has_associated_control -->
            <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Res</label>
            <select bind:value={conformResolution}
                    class="bg-transparent text-[12px] text-white outline-none font-mono tabular-nums">
              <option value="source">source</option>
              <option value="3840x2160">3840×2160 (UHD)</option>
              <option value="1920x1080">1920×1080 (HD)</option>
              <option value="1280x720">1280×720</option>
              <option value="854x480">854×480</option>
            </select>
          </div>
          <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
            <!-- svelte-ignore a11y_label_has_associated_control -->
            <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Pix</label>
            <select bind:value={conformPixFmt}
                    class="bg-transparent text-[12px] text-white outline-none font-mono tabular-nums">
              <option value="source">source</option>
              <option value="yuv420p">yuv420p (8-bit)</option>
              <option value="yuv420p10le">yuv420p10le (10-bit)</option>
              <option value="yuv422p">yuv422p (8-bit)</option>
              <option value="yuv422p10le">yuv422p10le (10-bit)</option>
              <option value="yuv444p">yuv444p (8-bit)</option>
            </select>
          </div>
          <button class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors"
                  title="Pick a reference video and copy its fps/resolution/pix_fmt into the fields above.">Match reference…</button>
        </div>
        <div class="flex flex-wrap items-center gap-2 w-full">
          <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)]">
            <span class="px-2 text-[10px] uppercase tracking-wider text-white/40 font-semibold">FPS algo</span>
            <div class="w-px h-6 bg-[var(--border)]"></div>
            <button onclick={() => conformFpsAlgo = 'drop'}
                    title="Drop / duplicate frames. Fast, deterministic. Judder on non-integer ratios."
                    class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                           {conformFpsAlgo === 'drop' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">Drop/dup</button>
            <div class="w-px h-6 bg-[var(--border)]"></div>
            <button onclick={() => conformFpsAlgo = 'blend'}
                    title="Blend adjacent frames. Smoother than drop/dup, but ghosting on motion."
                    class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                           {conformFpsAlgo === 'blend' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">Blend</button>
            <div class="w-px h-6 bg-[var(--border)]"></div>
            <button onclick={() => conformFpsAlgo = 'mci'}
                    title="Motion-compensated interpolation (minterpolate). Highest quality, 5–30× slower. May warp on complex motion / occlusions."
                    class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                           {conformFpsAlgo === 'mci' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}">
              Optical flow
              {#if conformFpsAlgo === 'mci'}<span class="ml-1 text-[9px] opacity-75">slow</span>{/if}
            </button>
          </div>
          <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
            <!-- svelte-ignore a11y_label_has_associated_control -->
            <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Scale</label>
            <select bind:value={conformScaleAlgo}
                    title="Bilinear: fast, soft. Bicubic: balanced. Lanczos: sharp downscale, ringing risk upscale. Spline: smoother upscale."
                    class="bg-transparent text-[12px] text-white outline-none">
              <option value="bilinear">Bilinear</option>
              <option value="bicubic">Bicubic</option>
              <option value="lanczos">Lanczos</option>
              <option value="spline">Spline</option>
            </select>
          </div>
          <label class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1 cursor-pointer"
                 title="Apply error-diffusion dither when converting 10-bit → 8-bit. Prevents banding in gradients. Auto-applied only if source and target depths differ.">
            <input type="checkbox" bind:checked={conformDither}
                   class="accent-[var(--accent)]"/>
            <span class="text-[11px] text-white/70 font-medium">10→8 dither</span>
          </label>
        </div>
        <div class="w-full">
          <button
            onclick={runConform}
            disabled={!selectedItem || selectedItem.mediaType !== 'video' || selectedItem.status === 'converting'}
            class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40 disabled:cursor-not-allowed"
          >Run Conform</button>
        </div>
      {:else if selectedOperation === 'silence-remove'}
        <div class="flex flex-wrap items-center gap-2 w-full">
          <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
            <!-- svelte-ignore a11y_label_has_associated_control -->
            <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Threshold (dB)</label>
            <input type="number" step="1" bind:value={silenceThresholdDb}
                   class="w-14 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
          </div>
          <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
            <!-- svelte-ignore a11y_label_has_associated_control -->
            <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Min silence (s)</label>
            <input type="number" step="0.05" bind:value={silenceMinDurS}
                   class="w-14 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
          </div>
          <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
            <!-- svelte-ignore a11y_label_has_associated_control -->
            <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Pad (ms)</label>
            <input type="number" step="10" bind:value={silencePadMs}
                   class="w-14 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
          </div>
        </div>
        <div class="w-full">
          <button
            onclick={runSilenceRemove}
            disabled={!selectedItem || selectedItem.status === 'converting'}
            class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
          >Run Silence Remover</button>
        </div>
      {:else if selectedOperation === 'remove-audio'}
        <div class="w-full">
          <button
            onclick={runRemoveAudio}
            disabled={!selectedItem || selectedItem.status === 'converting'}
            class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
          >Run Remove Audio</button>
        </div>
      {:else if selectedOperation === 'remove-video'}
        <div class="w-full">
          <button
            onclick={runRemoveVideo}
            disabled={!selectedItem || selectedItem.status === 'converting'}
            class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
          >Run Remove Video</button>
        </div>
      {:else if selectedOperation === 'metadata-strip'}
        <div class="flex flex-wrap items-center gap-2 w-full">
          <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)]">
            <button onclick={() => metadataStripMode = 'all'}
              class="px-3 py-1.5 text-[12px] font-semibold transition-colors
                     {metadataStripMode === 'all' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}"
            >Strip all</button>
            <div class="w-px h-6 bg-[var(--border)]"></div>
            <button onclick={() => metadataStripMode = 'title'}
              class="px-3 py-1.5 text-[12px] font-semibold transition-colors
                     {metadataStripMode === 'title' ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}"
            >Keep title only</button>
          </div>
          {#if metadataStripMode === 'title'}
            <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1 flex-1 min-w-[140px]">
              <!-- svelte-ignore a11y_label_has_associated_control -->
              <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Title</label>
              <input type="text" bind:value={metadataStripTitle}
                     placeholder="New title"
                     class="flex-1 bg-transparent text-[12px] text-white outline-none"/>
            </div>
          {/if}
        </div>
        <div class="w-full">
          <button
            onclick={runMetadataStrip}
            disabled={!selectedItem || selectedItem.status === 'converting'}
            class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
          >Run Strip Metadata</button>
        </div>
      {:else if selectedOperation === 'loop'}
        <div class="flex flex-wrap items-center gap-2 w-full">
          <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
            <!-- svelte-ignore a11y_label_has_associated_control -->
            <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Count</label>
            <input type="number" min="2" max="50" step="1" bind:value={loopCount}
                   class="w-14 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
          </div>
        </div>
        <div class="w-full">
          <button
            onclick={runLoop}
            disabled={!selectedItem || selectedItem.status === 'converting'}
            class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
          >Run Loop</button>
        </div>
      {:else if selectedOperation === 'rotate-flip'}
        <div class="flex flex-wrap items-center gap-2 w-full">
          <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)]">
            {#each [['cw90','90° CW'],['ccw90','90° CCW'],['180','180°'],['hflip','Flip H'],['vflip','Flip V']] as [id, label], i}
              {#if i > 0}<div class="w-px h-6 bg-[var(--border)]"></div>{/if}
              <button onclick={() => rotateFlipMode = id}
                class="px-3 py-1.5 text-[12px] font-semibold transition-colors
                       {rotateFlipMode === id ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}"
              >{label}</button>
            {/each}
          </div>
        </div>
        <div class="w-full">
          <button onclick={runRotateFlip}
            disabled={!selectedItem || selectedItem.mediaType !== 'video' || selectedItem.status === 'converting'}
            class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
          >Run Rotate/Flip</button>
        </div>
      {:else if selectedOperation === 'speed'}
        <div class="flex flex-wrap items-center gap-2 w-full">
          <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)]">
            {#each ['0.5','0.75','1','1.25','1.5','2','custom'] as p, i}
              {#if i > 0}<div class="w-px h-6 bg-[var(--border)]"></div>{/if}
              <button onclick={() => speedPreset = p}
                class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                       {speedPreset === p ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}"
              >{p === 'custom' ? 'Custom' : p + 'x'}</button>
            {/each}
          </div>
          {#if speedPreset === 'custom'}
            <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
              <!-- svelte-ignore a11y_label_has_associated_control -->
              <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Rate</label>
              <input type="number" min="0.1" max="10" step="0.05" bind:value={speedCustom}
                     class="w-16 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
            </div>
          {/if}
        </div>
        <div class="w-full flex items-center gap-2">
          <button onclick={runSpeed}
            disabled={!selectedItem || selectedItem.status === 'converting'}
            class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
          >Run Speed</button>
          <button onclick={runReverse}
            disabled={!selectedItem || selectedItem.status === 'converting'}
            class="px-3 py-1.5 rounded text-[12px] font-semibold border border-[var(--accent)] text-[var(--accent)] hover:bg-[var(--accent)] hover:text-white transition-colors disabled:opacity-40"
          >Run Reverse</button>
        </div>
      {:else if selectedOperation === 'fade'}
        <div class="flex flex-wrap items-center gap-2 w-full">
          <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
            <!-- svelte-ignore a11y_label_has_associated_control -->
            <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">In (s)</label>
            <input type="number" min="0" max="10" step="0.1" bind:value={fadeInS}
                   class="w-14 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
          </div>
          <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
            <!-- svelte-ignore a11y_label_has_associated_control -->
            <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Out (s)</label>
            <input type="number" min="0" max="10" step="0.1" bind:value={fadeOutS}
                   class="w-14 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
          </div>
        </div>
        <div class="w-full">
          <button onclick={runFade}
            disabled={!selectedItem || selectedItem.status === 'converting'}
            class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
          >Run Fade</button>
        </div>
      {:else if selectedOperation === 'deinterlace'}
        <div class="flex flex-wrap items-center gap-2 w-full">
          <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)]">
            {#each [['yadif','yadif'],['yadif_double','yadif 2×'],['bwdif','bwdif']] as [id, label], i}
              {#if i > 0}<div class="w-px h-6 bg-[var(--border)]"></div>{/if}
              <button onclick={() => deinterlaceMode = id}
                class="px-3 py-1.5 text-[12px] font-semibold transition-colors
                       {deinterlaceMode === id ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}"
              >{label}</button>
            {/each}
          </div>
        </div>
        <div class="w-full">
          <button onclick={runDeinterlace}
            disabled={!selectedItem || selectedItem.mediaType !== 'video' || selectedItem.status === 'converting'}
            class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
          >Run Deinterlace</button>
        </div>
      {:else if selectedOperation === 'denoise'}
        <div class="flex flex-wrap items-center gap-2 w-full">
          <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)]">
            {#each ['light','medium','strong'] as p, i}
              {#if i > 0}<div class="w-px h-6 bg-[var(--border)]"></div>{/if}
              <button onclick={() => denoisePreset = p}
                class="px-3 py-1.5 text-[12px] font-semibold capitalize transition-colors
                       {denoisePreset === p ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}"
              >{p}</button>
            {/each}
          </div>
        </div>
        <div class="w-full">
          <button onclick={runDenoise}
            disabled={!selectedItem || selectedItem.mediaType !== 'video' || selectedItem.status === 'converting'}
            class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
          >Run Denoise</button>
        </div>
      {:else if selectedOperation === 'thumbnail'}
        <div class="flex flex-wrap items-center gap-2 w-full">
          <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
            <!-- svelte-ignore a11y_label_has_associated_control -->
            <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Time</label>
            <input type="text" bind:value={thumbnailTime}
                   placeholder="00:00:01"
                   class="w-24 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
          </div>
          <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)]">
            {#each ['jpeg','png','webp'] as p, i}
              {#if i > 0}<div class="w-px h-6 bg-[var(--border)]"></div>{/if}
              <button onclick={() => thumbnailFormat = p}
                class="px-3 py-1.5 text-[12px] font-semibold uppercase transition-colors
                       {thumbnailFormat === p ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}"
              >{p}</button>
            {/each}
          </div>
        </div>
        <div class="w-full">
          <button onclick={runThumbnail}
            disabled={!selectedItem || selectedItem.status === 'converting'}
            class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
          >Run Thumbnail</button>
        </div>
      {:else if selectedOperation === 'contact-sheet'}
        <div class="flex flex-wrap items-center gap-2 w-full">
          <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
            <!-- svelte-ignore a11y_label_has_associated_control -->
            <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Cols</label>
            <input type="number" min="1" max="20" step="1" bind:value={contactGridCols}
                   class="w-12 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
          </div>
          <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
            <!-- svelte-ignore a11y_label_has_associated_control -->
            <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Rows</label>
            <input type="number" min="1" max="20" step="1" bind:value={contactGridRows}
                   class="w-12 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
          </div>
          <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
            <!-- svelte-ignore a11y_label_has_associated_control -->
            <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Frames</label>
            <input type="number" min="1" max="500" step="1" bind:value={contactFrames}
                   class="w-16 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
          </div>
        </div>
        <div class="w-full">
          <button onclick={runContactSheet}
            disabled={!selectedItem || selectedItem.mediaType !== 'video' || selectedItem.status === 'converting'}
            class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
          >Run Contact Sheet</button>
        </div>
      {:else if selectedOperation === 'frame-export'}
        <div class="flex flex-wrap items-center gap-2 w-full">
          <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)]">
            {#each [['fps','FPS'],['interval','Interval (s)']] as [id, label], i}
              {#if i > 0}<div class="w-px h-6 bg-[var(--border)]"></div>{/if}
              <button onclick={() => frameExportMode = id}
                class="px-3 py-1.5 text-[12px] font-semibold transition-colors
                       {frameExportMode === id ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}"
              >{label}</button>
            {/each}
          </div>
          {#if frameExportMode === 'fps'}
            <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
              <!-- svelte-ignore a11y_label_has_associated_control -->
              <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">FPS</label>
              <input type="number" min="0.01" step="0.1" bind:value={frameExportFps}
                     class="w-16 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
            </div>
          {:else}
            <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
              <!-- svelte-ignore a11y_label_has_associated_control -->
              <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Interval</label>
              <input type="number" min="0.01" step="0.5" bind:value={frameExportInterval}
                     class="w-16 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
              <span class="text-[10px] text-white/30">s</span>
            </div>
          {/if}
          <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)]">
            {#each ['jpeg','png','webp'] as p, i}
              {#if i > 0}<div class="w-px h-6 bg-[var(--border)]"></div>{/if}
              <button onclick={() => frameExportFormat = p}
                class="px-3 py-1.5 text-[12px] font-semibold uppercase transition-colors
                       {frameExportFormat === p ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}"
              >{p}</button>
            {/each}
          </div>
        </div>
        <div class="w-full">
          <button onclick={runFrameExport}
            disabled={!selectedItem || selectedItem.mediaType !== 'video' || selectedItem.status === 'converting'}
            class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
          >Run Frame Export</button>
        </div>
      {:else if selectedOperation === 'watermark'}
        <div class="flex flex-wrap items-center gap-2 w-full">
          <button
            onclick={() => watermarkPath
              ? (watermarkPath = null)
              : pickAuxFile?.((p) => watermarkPath = p, 'image/png,image/*')}
            class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors"
          >{watermarkPath ? 'Clear watermark' : 'Pick PNG…'}</button>
          <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)]">
            <span class="px-2 text-[10px] uppercase tracking-wider text-white/40 font-semibold">Corner</span>
            {#each [['tl','TL'],['tr','TR'],['bl','BL'],['br','BR'],['center','Ctr']] as [id, label]}
              <div class="w-px h-6 bg-[var(--border)]"></div>
              <button onclick={() => watermarkCorner = id}
                class="px-2.5 py-1.5 text-[11px] font-semibold transition-colors
                       {watermarkCorner === id ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}"
              >{label}</button>
            {/each}
          </div>
          <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
            <!-- svelte-ignore a11y_label_has_associated_control -->
            <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Opacity</label>
            <input type="range" min="0" max="1" step="0.05" bind:value={watermarkOpacity}
                   class="w-24 accent-[var(--accent)]"/>
            <span class="text-[11px] text-white/60 font-mono tabular-nums w-10 text-right">{Number(watermarkOpacity).toFixed(2)}</span>
          </div>
          <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
            <!-- svelte-ignore a11y_label_has_associated_control -->
            <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Size</label>
            <input type="number" min="1" max="100" step="1" bind:value={watermarkScale}
                   class="w-14 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
            <span class="text-[10px] text-white/30">%</span>
          </div>
        </div>
        <div class="w-full">
          <button onclick={runWatermark}
            disabled={!selectedItem || selectedItem.mediaType !== 'video' || !watermarkPath || selectedItem.status === 'converting'}
            class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
          >Run Watermark</button>
        </div>
      {:else if selectedOperation === 'channel-tools'}
        <div class="flex flex-wrap items-center gap-2 w-full">
          <div class="inline-flex items-center rounded-md overflow-hidden border border-[var(--border)] flex-wrap">
            {#each [['stereo_to_mono','Stereo→Mono'],['swap','Swap L↔R'],['mute_l','Mute L'],['mute_r','Mute R'],['mono_to_stereo','Mono→Stereo']] as [id, label], i}
              {#if i > 0}<div class="w-px h-6 bg-[var(--border)]"></div>{/if}
              <button onclick={() => channelToolsMode = id}
                class="px-3 py-1.5 text-[12px] font-semibold transition-colors
                       {channelToolsMode === id ? 'bg-[var(--accent)] text-white' : 'text-white/60 hover:bg-white/5'}"
              >{label}</button>
            {/each}
          </div>
        </div>
        <div class="w-full">
          <button onclick={runChannelTools}
            disabled={!selectedItem || selectedItem.status === 'converting'}
            class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
          >Run Channel Tools</button>
        </div>
      {:else if selectedOperation === 'pad-silence'}
        <div class="flex flex-wrap items-center gap-2 w-full">
          <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
            <!-- svelte-ignore a11y_label_has_associated_control -->
            <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Head (s)</label>
            <input type="number" min="0" max="60" step="0.1" bind:value={padSilenceHead}
                   class="w-14 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
          </div>
          <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
            <!-- svelte-ignore a11y_label_has_associated_control -->
            <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Tail (s)</label>
            <input type="number" min="0" max="60" step="0.1" bind:value={padSilenceTail}
                   class="w-14 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
          </div>
        </div>
        <div class="w-full">
          <button onclick={runPadSilence}
            disabled={!selectedItem || selectedItem.status === 'converting'}
            class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
          >Run Pad Silence</button>
        </div>
      {:else if selectedOperation === 'chroma-ffmpeg'}
        <ChromaKeyPanel
          bind:selectedItem
          videoOptions={videoOptions}
          outputDir={outputDir}
          outputSeparator={outputSeparator}
          setStatus={setStatus}
        />
      {:else if selectedOperation === 'loudness' || selectedOperation === 'audio-norm' || selectedOperation === 'cut-detect' || selectedOperation === 'black-detect' || selectedOperation === 'vmaf' || selectedOperation === 'framemd5'}
        <AnalysisTools
          bind:selectedItem
          selectedOperation={selectedOperation}
          outputDir={outputDir}
          outputSeparator={outputSeparator}
          pickAuxFile={pickAuxFile}
          setStatus={setStatus}
        />
      {:else if selectedOperation === 'merge'}
        <div class="flex flex-wrap items-center gap-2 w-full">
          <button
            onclick={() => {
              const _sid = selectedItem?.id;
              if (_sid && !mergeSelection.includes(_sid)) {
                mergeSelection = [...mergeSelection, _sid];
              }
            }}
            disabled={!selectedItem?.id || mergeSelection.includes(selectedItem?.id)}
            class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors disabled:opacity-40"
          >Add selected</button>
          <button
            onclick={() => mergeSelection = []}
            disabled={mergeSelection.length === 0}
            class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors disabled:opacity-40"
          >Clear list</button>
          <button
            onclick={runMerge}
            class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
            disabled={mergeSelection.length < 2}
          >Run Merge</button>
        </div>
        <div class="w-full rounded border border-[var(--border)] bg-black/20 px-2 py-1.5 max-h-48 overflow-y-auto">
          {#if mergeSelection.length === 0}
            <span class="text-white/30 text-[12px]">Select a file in the queue and click <em>Add selected</em>. Needs at least 2.</span>
          {:else}
            {#each mergeSelection as id, i (id)}
              {@const q = queue.find(x => x.id === id)}
              <div class="flex items-center gap-2 py-1">
                <span class="text-white/40 font-mono tabular-nums text-[10px] w-5">{i + 1}.</span>
                <span class="flex-1 truncate text-[12px] text-white/80">{q?.name ?? id}</span>
                <button
                  onclick={() => {
                    if (i === 0) return;
                    const next = [...mergeSelection];
                    [next[i - 1], next[i]] = [next[i], next[i - 1]];
                    mergeSelection = next;
                  }}
                  disabled={i === 0}
                  class="px-1.5 py-0.5 text-[11px] rounded border border-[var(--border)] text-white/60 hover:text-white hover:border-[var(--accent)] disabled:opacity-30"
                  title="Move up"
                >↑</button>
                <button
                  onclick={() => {
                    if (i === mergeSelection.length - 1) return;
                    const next = [...mergeSelection];
                    [next[i], next[i + 1]] = [next[i + 1], next[i]];
                    mergeSelection = next;
                  }}
                  disabled={i === mergeSelection.length - 1}
                  class="px-1.5 py-0.5 text-[11px] rounded border border-[var(--border)] text-white/60 hover:text-white hover:border-[var(--accent)] disabled:opacity-30"
                  title="Move down"
                >↓</button>
                <button
                  onclick={() => mergeSelection = mergeSelection.filter(x => x !== id)}
                  class="px-1.5 py-0.5 text-[11px] rounded border border-[var(--border)] text-white/60 hover:text-red-400 hover:border-red-400"
                  title="Remove"
                >×</button>
              </div>
            {/each}
          {/if}
        </div>
      {:else if selectedOperation === 'extract'}
        {#each [
          { id: 'video',    label: 'Video' },
          { id: 'audio',    label: 'Audio' },
          { id: 'subtitle', label: 'Subtitles' },
          { id: 'all',      label: 'All streams' },
        ] as m}
          <button
            onclick={() => extractMode = m.id}
            class="px-3 py-1.5 rounded text-[12px] font-semibold border transition-colors
                   {extractMode === m.id
                     ? 'bg-[var(--accent)] border-[var(--accent)] text-white'
                     : 'border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)]'}"
          >{m.label}</button>
        {/each}
        <button
          onclick={runExtract}
          disabled={!selectedItem || selectedItem.status === 'converting'}
          class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
        >Run Extract</button>
      {:else if selectedOperation === 'subtitling'}
        <div class="flex flex-col gap-3 w-full">
          <div class="flex gap-1 border-b border-[var(--border)] w-full">
            {#each [
              { id: 'embed',    label: 'Embed / Burn-in' },
              { id: 'generate', label: 'Generate (AI)' },
              { id: 'analyze',  label: 'Analyze' },
            ] as tab}
              <button
                onclick={() => subtitlingTab = tab.id}
                class="px-3 py-1.5 text-[12px] font-medium -mb-px border-b-2 transition-colors
                       {subtitlingTab === tab.id
                         ? 'border-[var(--accent)] text-white'
                         : 'border-transparent text-white/50 hover:text-white/80'}"
              >{tab.label}</button>
            {/each}
          </div>

          {#if subtitlingTab === 'embed'}
            <div class="flex flex-wrap gap-2">
              <button class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">Pick subtitle file…</button>
              <button class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">Burn-in</button>
              <button class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">Embed (soft)</button>
              <button class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">Style…</button>
              <button class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity">Run Subtitling</button>
            </div>
          {:else if subtitlingTab === 'generate'}
            <div class="flex flex-wrap gap-2">
              <button class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">Transcribe (Whisper)</button>
              <button class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">Translate SRT…</button>
              <button class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">Forced Align</button>
              <button class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">Diarize Speakers</button>
              <button class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">OCR Burned Subs</button>
              <button class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">Restore Punctuation</button>
            </div>
          {:else if subtitlingTab === 'analyze'}
            <div class="flex flex-wrap items-center gap-2 w-full">
              <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1"
                   title="Characters-per-second reading speed ceiling. >21 is too fast to read comfortably.">
                <!-- svelte-ignore a11y_label_has_associated_control -->
                <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">CPS max</label>
                <input type="number" step="1" bind:value={subLintCpsMax}
                       class="w-12 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
              </div>
              <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                <!-- svelte-ignore a11y_label_has_associated_control -->
                <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Min dur (ms)</label>
                <input type="number" step="100" bind:value={subLintMinDurMs}
                       class="w-16 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
              </div>
              <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                <!-- svelte-ignore a11y_label_has_associated_control -->
                <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Max dur (ms)</label>
                <input type="number" step="100" bind:value={subLintMaxDurMs}
                       class="w-16 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
              </div>
              <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                <!-- svelte-ignore a11y_label_has_associated_control -->
                <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Line max</label>
                <input type="number" step="1" bind:value={subLintLineMaxChars}
                       class="w-12 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
              </div>
              <div class="flex items-center gap-1.5 rounded border border-[var(--border)] px-2 py-1">
                <!-- svelte-ignore a11y_label_has_associated_control -->
                <label class="text-[10px] uppercase tracking-wider text-white/40 font-semibold">Lines max</label>
                <input type="number" step="1" bind:value={subLintMaxLines}
                       class="w-10 bg-transparent text-[12px] text-white outline-none text-right font-mono tabular-nums"/>
              </div>
            </div>
            <div class="flex flex-wrap items-center gap-2 w-full">
              <button
                onclick={runSubLint}
                disabled={!selectedItem}
                class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
              >Run Lint</button>
              <button onclick={() => subDiffReferencePath
                        ? (subDiffReferencePath = null)
                        : pickAuxFile?.((p) => subDiffReferencePath = p, '.srt,.vtt,.ass,.ssa')}
                      title="Optional reference subtitle for a diff. Without one, Lint runs on the selected file."
                      class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">
                {subDiffReferencePath ? 'Clear diff reference' : 'Pick diff reference…'}
              </button>
              {#if subDiffReferencePath}
                <button
                  onclick={runSubDiff}
                  disabled={!selectedItem}
                  class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-40"
                >Run Diff</button>
              {/if}
              <button
                onclick={runSubProbe}
                disabled={!selectedItem}
                class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors disabled:opacity-40"
                title="Probe the currently selected video for subtitle streams (ffprobe).">Detect tracks</button>
            </div>

            {#if subLintResults}
              <div class="w-full rounded border border-[var(--border)] bg-black/20 px-3 py-2 text-[11px] font-mono max-h-48 overflow-y-auto">
                {#if subLintResults.length === 0}
                  <span class="text-white/30">No lint issues.</span>
                {:else}
                  {#each subLintResults as issue}
                    <div class="flex gap-2 py-0.5">
                      <span class="text-white/40 w-20 shrink-0">{issue.time}</span>
                      <span class="text-[var(--accent)] w-28 shrink-0">{issue.kind}</span>
                      <span class="text-white/70">{issue.message}</span>
                    </div>
                  {/each}
                {/if}
              </div>
            {/if}
            {#if subDiffResults}
              <div class="w-full rounded border border-[var(--border)] bg-black/20 px-3 py-2 text-[11px] font-mono max-h-48 overflow-y-auto">
                {#each subDiffResults as d}
                  <div class="flex gap-2 py-0.5
                              {d.kind === 'add' ? 'text-emerald-400' : d.kind === 'del' ? 'text-red-400' : 'text-white/50'}">
                    <span class="w-4 shrink-0">{d.kind === 'add' ? '+' : d.kind === 'del' ? '-' : ' '}</span>
                    <span class="truncate">{d.left ?? d.right ?? ''}</span>
                  </div>
                {/each}
              </div>
            {/if}
            {#if subProbeResults}
              <div class="w-full rounded border border-[var(--border)] bg-black/20 px-3 py-2 text-[11px] font-mono max-h-40 overflow-y-auto">
                {#if subProbeResults.length === 0}
                  <span class="text-white/30">No subtitle streams found.</span>
                {:else}
                  {#each subProbeResults as s}
                    <div class="flex gap-2 py-0.5">
                      <span class="text-white/40 w-10 shrink-0">#{s.index}</span>
                      <span class="text-white/70 w-24 shrink-0">{s.codec}</span>
                      <span class="text-white/60 w-20 shrink-0">{s.language ?? '—'}</span>
                      <span class="text-white/50 truncate">{s.title ?? ''}</span>
                    </div>
                  {/each}
                {/if}
              </div>
            {/if}
          {/if}
        </div>
      {:else if selectedOperation === 'video-inserts'}
        <button class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">Set insert point</button>
        <button class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">Pick insert clip…</button>
        <button class="px-3 py-1.5 rounded text-[12px] font-medium border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:border-[var(--accent)] transition-colors">Trim insert</button>
        <button class="px-3 py-1.5 rounded text-[12px] font-semibold bg-[var(--accent)] text-white hover:opacity-90 transition-opacity">Run Insert</button>
      {/if}
    </div>
  </div>
</div>
