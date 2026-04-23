import { test, expect } from '@playwright/experimental-ct-svelte'
import FormatPickerWrapper from '../wrappers/FormatPickerWrapper.svelte'

test('FormatPicker renders all format buttons', async ({ mount }) => {
  const component = await mount(FormatPickerWrapper, {
    props: {
      formats: ['jpg', 'png', 'webp'],
      initialFormat: 'jpg',
    },
  })
  await expect(component.getByRole('button', { name: 'JPG' })).toBeVisible()
  await expect(component.getByRole('button', { name: 'PNG' })).toBeVisible()
  await expect(component.getByRole('button', { name: 'WEBP' })).toBeVisible()
})

test('FormatPicker initial format button has seg-active class', async ({ mount }) => {
  const component = await mount(FormatPickerWrapper, {
    props: {
      formats: ['jpg', 'png', 'webp'],
      initialFormat: 'jpg',
    },
  })
  const jpgBtn = component.getByRole('button', { name: 'JPG' })
  await expect(jpgBtn).toHaveClass(/seg-active/)
})

test('FormatPicker clicking png makes it active', async ({ mount }) => {
  const component = await mount(FormatPickerWrapper, {
    props: {
      formats: ['jpg', 'png', 'webp'],
      initialFormat: 'jpg',
    },
  })
  const pngBtn = component.getByRole('button', { name: 'PNG' })
  await pngBtn.click()
  await expect(pngBtn).toHaveClass(/seg-active/)
})

test('FormatPicker clicking webp makes it active and deactivates jpg', async ({ mount }) => {
  const component = await mount(FormatPickerWrapper, {
    props: {
      formats: ['jpg', 'png', 'webp'],
      initialFormat: 'jpg',
    },
  })
  const jpgBtn  = component.getByRole('button', { name: 'JPG' })
  const webpBtn = component.getByRole('button', { name: 'WEBP' })

  await webpBtn.click()
  await expect(webpBtn).toHaveClass(/seg-active/)
  await expect(jpgBtn).not.toHaveClass(/seg-active/)
})

test('FormatPicker cycles through all three formats', async ({ mount }) => {
  const component = await mount(FormatPickerWrapper, {
    props: {
      formats: ['jpg', 'png', 'webp'],
      initialFormat: 'jpg',
    },
  })
  for (const fmt of ['PNG', 'WEBP', 'JPG']) {
    const btn = component.getByRole('button', { name: fmt })
    await btn.click()
    await expect(btn).toHaveClass(/seg-active/)
  }
})
