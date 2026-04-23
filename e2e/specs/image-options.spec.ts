import { test, expect } from '@playwright/experimental-ct-svelte'
import ImageOptionsWrapper from '../wrappers/ImageOptionsWrapper.svelte'

const baseOpts = {
  output_format: 'jpeg',
  quality: 80,
  resize_mode: 'none',
  resize_percent: 100,
  resize_width: null,
  resize_height: null,
  rotation: 0,
  flip_h: false,
  flip_v: false,
  auto_rotate: false,
  preserve_metadata: false,
  crop_x: null,
  crop_y: null,
  crop_width: null,
  crop_height: null,
  jpeg_chroma: '420',
  jpeg_progressive: false,
}

test('ImageOptions quality slider visible for jpeg format', async ({ mount }) => {
  const component = await mount(ImageOptionsWrapper, {
    props: { initialOptions: { ...baseOpts } },
  })
  const slider = component.locator('input.fade-range').first()
  await expect(slider).toBeVisible()
})

test('ImageOptions quality slider min/max are 5 and 100', async ({ mount }) => {
  const component = await mount(ImageOptionsWrapper, {
    props: { initialOptions: { ...baseOpts } },
  })
  const slider = component.locator('input.fade-range').first()
  await expect(slider).toHaveAttribute('min', '5')
  await expect(slider).toHaveAttribute('max', '100')
})

test('ImageOptions resize mode none — pixel inputs not visible', async ({ mount }) => {
  const component = await mount(ImageOptionsWrapper, {
    props: { initialOptions: { ...baseOpts, resize_mode: 'none' } },
  })
  await expect(component.locator('#img-w')).not.toBeVisible()
})

test('ImageOptions resize mode percent shows scale slider', async ({ mount }) => {
  const component = await mount(ImageOptionsWrapper, {
    props: { initialOptions: { ...baseOpts, resize_mode: 'percent' } },
  })
  // Quality slider + scale slider = 2
  const sliders = component.locator('input.fade-range')
  await expect(sliders).toHaveCount(2)
})

test('ImageOptions clicking Percentage resize mode makes it active', async ({ mount }) => {
  const component = await mount(ImageOptionsWrapper, {
    props: { initialOptions: { ...baseOpts } },
  })
  const pctBtn = component.getByRole('button', { name: 'Percentage' })
  await pctBtn.click()
  await expect(pctBtn).toHaveClass(/seg-active/)
})

test('ImageOptions clicking Pixel dimensions shows width/height inputs', async ({ mount }) => {
  const component = await mount(ImageOptionsWrapper, {
    props: { initialOptions: { ...baseOpts } },
  })
  const pixBtn = component.getByRole('button', { name: 'Pixel dimensions' })
  await pixBtn.click()
  await expect(component.locator('#img-w')).toBeVisible()
  await expect(component.locator('#img-h')).toBeVisible()
})

test('ImageOptions clicking No resize hides dimension inputs', async ({ mount }) => {
  const component = await mount(ImageOptionsWrapper, {
    props: { initialOptions: { ...baseOpts, resize_mode: 'pixels', resize_width: 1920, resize_height: 1080 } },
  })
  const noneBtn = component.getByRole('button', { name: 'No resize' })
  await noneBtn.click()
  await expect(component.locator('#img-w')).not.toBeVisible()
})

test('ImageOptions crop preset Free button is visible and clickable', async ({ mount }) => {
  const component = await mount(ImageOptionsWrapper, {
    props: {
      initialOptions: { ...baseOpts },
      cropActive: false,
    },
  })
  const freeBtn = component.getByRole('button', { name: 'Free' })
  await expect(freeBtn).toBeVisible()
  await freeBtn.click()
  // button remains visible after click
  await expect(freeBtn).toBeVisible()
})

test('ImageOptions rotation buttons present and 90° click makes it active', async ({ mount }) => {
  const component = await mount(ImageOptionsWrapper, {
    props: { initialOptions: { ...baseOpts } },
  })
  await expect(component.getByRole('button', { name: 'None' })).toBeVisible()
  const rot90 = component.getByRole('button', { name: '90°' })
  await rot90.click()
  await expect(rot90).toHaveClass(/seg-active/)
})

test('ImageOptions None rotation is initially active', async ({ mount }) => {
  const component = await mount(ImageOptionsWrapper, {
    props: { initialOptions: { ...baseOpts } },
  })
  await expect(component.getByRole('button', { name: 'None' })).toHaveClass(/seg-active/)
})

test('ImageOptions clicking 180° rotation makes it active', async ({ mount }) => {
  const component = await mount(ImageOptionsWrapper, {
    props: { initialOptions: { ...baseOpts } },
  })
  const rot180 = component.getByRole('button', { name: '180°' })
  await rot180.click()
  await expect(rot180).toHaveClass(/seg-active/)
})
