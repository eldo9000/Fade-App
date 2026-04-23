import { test, expect } from '@playwright/experimental-ct-svelte'
import AudioOptionsWrapper from '../wrappers/AudioOptionsWrapper.svelte'

const baseOpts = {
  output_format: 'mp3',
  bitrate: 128,
  sample_rate: 44100,
  trim_start: null,
  trim_end: null,
  preserve_metadata: false,
  mp3_bitrate_mode: 'cbr',
  channels: 'stereo',
  normalize_loudness: false,
  dsp_limiter_db: null,
  dsp_highpass_freq: null,
  dsp_lowpass_freq: null,
  dsp_stereo_width: null,
  pad_front: null,
  pad_end: null,
}

test('AudioOptions renders bitrate buttons', async ({ mount }) => {
  const component = await mount(AudioOptionsWrapper, {
    props: { initialOptions: { ...baseOpts } },
  })
  await expect(component.getByRole('button', { name: '128', exact: true })).toBeVisible()
  await expect(component.getByRole('button', { name: '192', exact: true })).toBeVisible()
  await expect(component.getByRole('button', { name: '320', exact: true })).toBeVisible()
})

test('AudioOptions initial bitrate 128 is active', async ({ mount }) => {
  const component = await mount(AudioOptionsWrapper, {
    props: { initialOptions: { ...baseOpts } },
  })
  await expect(component.getByRole('button', { name: '128' })).toHaveClass(/seg-active/)
})

test('AudioOptions clicking 192 bitrate makes it active', async ({ mount }) => {
  const component = await mount(AudioOptionsWrapper, {
    props: { initialOptions: { ...baseOpts } },
  })
  const btn192 = component.getByRole('button', { name: '192', exact: true })
  await btn192.click()
  await expect(btn192).toHaveClass(/seg-active/)
})

test('AudioOptions clicking 320 bitrate deactivates 128', async ({ mount }) => {
  const component = await mount(AudioOptionsWrapper, {
    props: { initialOptions: { ...baseOpts } },
  })
  const btn128 = component.getByRole('button', { name: '128' })
  const btn320 = component.getByRole('button', { name: '320' })
  await btn320.click()
  await expect(btn320).toHaveClass(/seg-active/)
  await expect(btn128).not.toHaveClass(/seg-active/)
})

test('AudioOptions renders sample rate buttons', async ({ mount }) => {
  const component = await mount(AudioOptionsWrapper, {
    props: { initialOptions: { ...baseOpts } },
  })
  await expect(component.getByRole('button', { name: /44\.1 kHz/ })).toBeVisible()
  await expect(component.getByRole('button', { name: /48 kHz/ })).toBeVisible()
})

test('AudioOptions initial 44.1kHz sample rate is active', async ({ mount }) => {
  const component = await mount(AudioOptionsWrapper, {
    props: { initialOptions: { ...baseOpts } },
  })
  await expect(component.getByRole('button', { name: /44\.1 kHz/ })).toHaveClass(/seg-active/)
})

test('AudioOptions clicking 48 kHz sample rate makes it active', async ({ mount }) => {
  const component = await mount(AudioOptionsWrapper, {
    props: { initialOptions: { ...baseOpts } },
  })
  const btn48 = component.getByRole('button', { name: /48 kHz/ })
  await btn48.click()
  await expect(btn48).toHaveClass(/seg-active/)
})

test('AudioOptions trim inputs accessible after expanding Length', async ({ mount }) => {
  const component = await mount(AudioOptionsWrapper, {
    props: { initialOptions: { ...baseOpts } },
  })
  const lengthBtn = component.getByRole('button', { name: 'Length' })
  await lengthBtn.click()
  await expect(component.locator('#aud-trim-start')).toBeVisible()
  await expect(component.locator('#aud-trim-end')).toBeVisible()
})

test('AudioOptions trim start input accepts a value and reformats it', async ({ mount }) => {
  const component = await mount(AudioOptionsWrapper, {
    props: { initialOptions: { ...baseOpts } },
  })
  await component.getByRole('button', { name: 'Length' }).click()
  const trimStart = component.locator('#aud-trim-start')
  await trimStart.fill('5')
  // Component reformats seconds to MM:SS — 5s → '0:05.0' (seconds zero-padded to 4 chars)
  await expect(trimStart).toHaveValue('0:05.0')
})

test('AudioOptions trim end input accepts a value and reformats it', async ({ mount }) => {
  const component = await mount(AudioOptionsWrapper, {
    props: { initialOptions: { ...baseOpts } },
  })
  await component.getByRole('button', { name: 'Length' }).click()
  const trimEnd = component.locator('#aud-trim-end')
  await trimEnd.fill('30')
  // Component reformats seconds to MM:SS — 30s → '0:30.0'
  await expect(trimEnd).toHaveValue('0:30.0')
})
