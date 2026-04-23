import { test, expect } from '@playwright/experimental-ct-svelte'
import VideoOptionsWrapper from '../wrappers/VideoOptionsWrapper.svelte'

const baseOpts = {
  output_format: 'mp4',
  codec: 'h264',
  crf: 23,
  preset: 'medium',
  resolution: 'original',
  frame_rate: 'original',
  video_bitrate_mode: 'crf',
  remove_audio: false,
  extract_audio: false,
  bitrate: 128,
  sample_rate: 48000,
  preserve_metadata: false,
  trim_start: null,
  trim_end: null,
  h264_profile: 'main',
  pix_fmt: 'yuv420p',
  tune: 'none',
}

test('VideoOptions renders the codec dropdown trigger with h264 label', async ({ mount }) => {
  const component = await mount(VideoOptionsWrapper, {
    props: { initialOptions: { ...baseOpts } },
  })
  await expect(component.locator('text=H.264 (AVC)')).toBeVisible()
})

test('VideoOptions codec dropdown opens and shows H.265', async ({ mount }) => {
  const component = await mount(VideoOptionsWrapper, {
    props: { initialOptions: { ...baseOpts } },
  })
  const codecTrigger = component
    .locator('fieldset')
    .filter({ hasText: 'Video Codec' })
    .locator('button')
    .first()
  await codecTrigger.click()
  await expect(component.locator('text=H.265 (HEVC)')).toBeVisible()
})

test('VideoOptions selecting H.265 from dropdown reflects in UI', async ({ mount }) => {
  const component = await mount(VideoOptionsWrapper, {
    props: { initialOptions: { ...baseOpts } },
  })
  const codecTrigger = component
    .locator('fieldset')
    .filter({ hasText: 'Video Codec' })
    .locator('button')
    .first()
  await codecTrigger.click()

  const h265Option = component.locator('button', { hasText: 'H.265 (HEVC)' })
  await h265Option.click()

  // Dropdown closed; trigger now shows H.265 label
  await expect(component.locator('text=H.265 (HEVC)')).toBeVisible()
})

test('VideoOptions quality slider present for h264 crf mode', async ({ mount }) => {
  const component = await mount(VideoOptionsWrapper, {
    props: { initialOptions: { ...baseOpts } },
  })
  const qualitySlider = component
    .locator('fieldset')
    .filter({ hasText: 'Quality' })
    .locator('input.fade-range')
  await expect(qualitySlider).toBeVisible()
  await expect(qualitySlider).toHaveAttribute('min', '0')
  await expect(qualitySlider).toHaveAttribute('max', '51')
})

test('VideoOptions resolution Original preset is visible', async ({ mount }) => {
  const component = await mount(VideoOptionsWrapper, {
    props: { initialOptions: { ...baseOpts } },
  })
  await expect(component.getByRole('button', { name: 'Original' })).toBeVisible()
})

test('VideoOptions clicking 1080p sets it as active resolution', async ({ mount }) => {
  const component = await mount(VideoOptionsWrapper, {
    props: { initialOptions: { ...baseOpts } },
  })
  const btn1080p = component.getByRole('button', { name: '1080p' })
  await btn1080p.click()
  // After clicking, the 1080p button should carry the active style
  // The active style is bg-[color-mix(in_srgb,var(--accent)_37.5%,#000)] — check it no longer has seg-inactive
  await expect(btn1080p).not.toHaveClass(/seg-inactive/)
})

test('VideoOptions Advanced section expands to show Encode Preset', async ({ mount }) => {
  const component = await mount(VideoOptionsWrapper, {
    props: { initialOptions: { ...baseOpts } },
  })
  const advancedBtn = component.getByRole('button', { name: 'Advanced' })
  await advancedBtn.click()
  await expect(
    component.locator('fieldset').filter({ hasText: 'Encode Preset' })
  ).toBeVisible()
})

test('VideoOptions preset dropdown opens and slow can be selected', async ({ mount }) => {
  const component = await mount(VideoOptionsWrapper, {
    props: { initialOptions: { ...baseOpts } },
  })
  await component.getByRole('button', { name: 'Advanced' }).click()

  const presetTrigger = component
    .locator('fieldset')
    .filter({ hasText: 'Encode Preset' })
    .locator('button')
    .first()
  await presetTrigger.click()

  const slowOption = component.getByRole('button', { name: 'slow', exact: true })
  await slowOption.click()

  // The preset trigger now shows 'slow'
  await expect(
    component
      .locator('fieldset')
      .filter({ hasText: 'Encode Preset' })
      .locator('span')
      .filter({ hasText: 'slow' })
  ).toBeVisible()
})

test('VideoOptions frame rate buttons visible in advanced section', async ({ mount }) => {
  const component = await mount(VideoOptionsWrapper, {
    props: { initialOptions: { ...baseOpts } },
  })
  await component.getByRole('button', { name: 'Advanced' }).click()
  await expect(component.getByRole('button', { name: '24', exact: true })).toBeVisible()
  await expect(component.getByRole('button', { name: '30', exact: true })).toBeVisible()
  await expect(component.getByRole('button', { name: '60', exact: true })).toBeVisible()
})

test('VideoOptions clicking 24 frame rate makes it active', async ({ mount }) => {
  const component = await mount(VideoOptionsWrapper, {
    props: { initialOptions: { ...baseOpts } },
  })
  await component.getByRole('button', { name: 'Advanced' }).click()
  const btn24 = component.getByRole('button', { name: '24', exact: true })
  await btn24.click()
  await expect(btn24).toHaveClass(/seg-active/)
})
