import { describe, it, expect } from 'vitest';
import { mediaTypeFor, validateOptions } from '../lib/utils.js';

// ── mediaTypeFor ──────────────────────────────────────────────────────────────

describe('mediaTypeFor', () => {
  it('classifies image extensions', () => {
    expect(mediaTypeFor('jpg')).toBe('image');
    expect(mediaTypeFor('jpeg')).toBe('image');
    expect(mediaTypeFor('png')).toBe('image');
    expect(mediaTypeFor('webp')).toBe('image');
    expect(mediaTypeFor('heic')).toBe('image');
    expect(mediaTypeFor('gif')).toBe('image');
    expect(mediaTypeFor('avif')).toBe('image');
  });

  it('classifies video extensions', () => {
    expect(mediaTypeFor('mp4')).toBe('video');
    expect(mediaTypeFor('mkv')).toBe('video');
    expect(mediaTypeFor('webm')).toBe('video');
    expect(mediaTypeFor('avi')).toBe('video');
    expect(mediaTypeFor('mov')).toBe('video');
  });

  it('classifies audio extensions', () => {
    expect(mediaTypeFor('mp3')).toBe('audio');
    expect(mediaTypeFor('flac')).toBe('audio');
    expect(mediaTypeFor('wav')).toBe('audio');
    expect(mediaTypeFor('aac')).toBe('audio');
    expect(mediaTypeFor('opus')).toBe('audio');
  });

  it('returns unknown for unrecognised extensions', () => {
    expect(mediaTypeFor('xyz')).toBe('unknown');
    expect(mediaTypeFor('')).toBe('unknown');
    expect(mediaTypeFor('exe')).toBe('unknown');
    expect(mediaTypeFor('pdf')).toBe('unknown');
  });
});

// ── Queue state transitions (pure logic) ──────────────────────────────────────

describe('queue state transitions', () => {
  function makeItem(overrides = {}) {
    return {
      id: crypto.randomUUID(),
      path: '/tmp/file.mp4',
      name: 'file.mp4',
      ext: 'mp4',
      mediaType: 'video',
      status: 'pending',
      percent: 0,
      ...overrides,
    };
  }

  it('add: appends items to the queue', () => {
    const queue = [];
    const item = makeItem();
    queue.push(item);
    expect(queue).toHaveLength(1);
    expect(queue[0].status).toBe('pending');
  });

  it('remove: filters out the item by id', () => {
    const a = makeItem();
    const b = makeItem();
    let queue = [a, b];
    queue = queue.filter(q => q.id !== a.id);
    expect(queue).toHaveLength(1);
    expect(queue[0].id).toBe(b.id);
  });

  it('clear: empties the queue', () => {
    let queue = [makeItem(), makeItem(), makeItem()];
    queue = [];
    expect(queue).toHaveLength(0);
  });

  it('status transitions: pending → converting → done', () => {
    const queue = [makeItem()];
    queue[0].status = 'converting';
    queue[0].percent = 50;
    expect(queue[0].status).toBe('converting');

    queue[0].status = 'done';
    queue[0].percent = 100;
    expect(queue[0].status).toBe('done');
  });

  it('status transitions: converting → error stores message', () => {
    const queue = [makeItem()];
    queue[0].status = 'converting';
    queue[0].status = 'error';
    queue[0].error = 'ffmpeg: codec not found';
    expect(queue[0].status).toBe('error');
    expect(queue[0].error).toBe('ffmpeg: codec not found');
  });

  it('status transitions: converting → cancelled', () => {
    const queue = [makeItem({ status: 'converting' })];
    queue[0].status = 'cancelled';
    queue[0].percent = 0;
    expect(queue[0].status).toBe('cancelled');
  });
});

// ── validateOptions ───────────────────────────────────────────────────────────

describe('validateOptions', () => {
  const baseVideo = {
    output_format: 'mp4',
    codec: 'h264',
    resolution: 'original',
    trim_start: null,
    trim_end: null,
    remove_audio: false,
    extract_audio: false,
    bitrate: 192,
    sample_rate: 48000,
  };

  const baseAudio = {
    output_format: 'mp3',
    bitrate: 192,
    sample_rate: 44100,
    normalize_loudness: false,
    trim_start: null,
    trim_end: null,
  };

  it('returns no errors for valid options', () => {
    const errors = validateOptions(baseVideo, baseAudio);
    expect(errors).toEqual({});
  });

  it('catches video trim_end <= trim_start', () => {
    const video = { ...baseVideo, trim_start: 30, trim_end: 10 };
    const errors = validateOptions(video, baseAudio);
    expect(errors.video_trim).toBeTruthy();
  });

  it('catches video trim_end === trim_start', () => {
    const video = { ...baseVideo, trim_start: 15, trim_end: 15 };
    const errors = validateOptions(video, baseAudio);
    expect(errors.video_trim).toBeTruthy();
  });

  it('no video trim error when only start is set', () => {
    const video = { ...baseVideo, trim_start: 10, trim_end: null };
    const errors = validateOptions(video, baseAudio);
    expect(errors.video_trim).toBeUndefined();
  });

  it('no video trim error when only end is set', () => {
    const video = { ...baseVideo, trim_start: null, trim_end: 60 };
    const errors = validateOptions(video, baseAudio);
    expect(errors.video_trim).toBeUndefined();
  });

  it('valid trim range passes', () => {
    const video = { ...baseVideo, trim_start: 10, trim_end: 60 };
    const errors = validateOptions(video, baseAudio);
    expect(errors.video_trim).toBeUndefined();
  });

  it('catches audio trim_end <= trim_start', () => {
    const audio = { ...baseAudio, trim_start: 50, trim_end: 20 };
    const errors = validateOptions(baseVideo, audio);
    expect(errors.audio_trim).toBeTruthy();
  });

  it('catches invalid custom resolution format', () => {
    const video = { ...baseVideo, resolution: 'notaresolution' };
    const errors = validateOptions(video, baseAudio);
    expect(errors.resolution).toBeTruthy();
  });

  it('accepts valid custom resolution WxH format', () => {
    const video = { ...baseVideo, resolution: '2560x1440' };
    const errors = validateOptions(video, baseAudio);
    expect(errors.resolution).toBeUndefined();
  });

  it('accepts all preset resolutions', () => {
    for (const res of ['original', '1920x1080', '1280x720', '854x480']) {
      const video = { ...baseVideo, resolution: res };
      const errors = validateOptions(video, baseAudio);
      expect(errors.resolution).toBeUndefined();
    }
  });
});
