import { test, expect } from '@playwright/experimental-ct-svelte'
import ArchiveOptionsWrapper from '../wrappers/ArchiveOptionsWrapper.svelte'

test('ArchiveOptions renders format buttons', async ({ mount }) => {
  const component = await mount(ArchiveOptionsWrapper, {
    props: { initialOptions: { output_format: 'zip', archive_compression: 5 } },
  })
  await expect(component.getByRole('button', { name: 'zip' })).toBeVisible()
  await expect(component.getByRole('button', { name: 'tar' })).toBeVisible()
  await expect(component.getByRole('button', { name: 'gz' })).toBeVisible()
  await expect(component.getByRole('button', { name: '7z' })).toBeVisible()
})

test('ArchiveOptions zip is the active format by default', async ({ mount }) => {
  const component = await mount(ArchiveOptionsWrapper, {
    props: { initialOptions: { output_format: 'zip', archive_compression: 5 } },
  })
  await expect(component.getByRole('button', { name: 'zip' })).toHaveClass(/seg-active/)
})

test('ArchiveOptions compression slider visible for zip with correct range', async ({ mount }) => {
  const component = await mount(ArchiveOptionsWrapper, {
    props: { initialOptions: { output_format: 'zip', archive_compression: 5 } },
  })
  const slider = component.locator('input.fade-range')
  await expect(slider).toBeVisible()
  await expect(slider).toHaveAttribute('min', '0')
  await expect(slider).toHaveAttribute('max', '9')
})

test('ArchiveOptions compression slider initial value is 5', async ({ mount }) => {
  const component = await mount(ArchiveOptionsWrapper, {
    props: { initialOptions: { output_format: 'zip', archive_compression: 5 } },
  })
  const slider = component.locator('input.fade-range')
  await expect(slider).toHaveValue('5')
})

test('ArchiveOptions compression slider can be set to min (0)', async ({ mount }) => {
  const component = await mount(ArchiveOptionsWrapper, {
    props: { initialOptions: { output_format: 'zip', archive_compression: 5 } },
  })
  const slider = component.locator('input.fade-range')
  await slider.fill('0')
  await expect(slider).toHaveValue('0')
})

test('ArchiveOptions compression slider can be set to max (9)', async ({ mount }) => {
  const component = await mount(ArchiveOptionsWrapper, {
    props: { initialOptions: { output_format: 'zip', archive_compression: 5 } },
  })
  const slider = component.locator('input.fade-range')
  await slider.fill('9')
  await expect(slider).toHaveValue('9')
})

test('ArchiveOptions compression slider not visible for tar format', async ({ mount }) => {
  const component = await mount(ArchiveOptionsWrapper, {
    props: { initialOptions: { output_format: 'tar', archive_compression: 5 } },
  })
  await expect(component.locator('input.fade-range')).not.toBeVisible()
})

test('ArchiveOptions clicking 7z makes it the active format', async ({ mount }) => {
  const component = await mount(ArchiveOptionsWrapper, {
    props: { initialOptions: { output_format: 'zip', archive_compression: 5 } },
  })
  const btn7z = component.getByRole('button', { name: '7z' })
  await btn7z.click()
  await expect(btn7z).toHaveClass(/seg-active/)
})

test('ArchiveOptions clicking gz makes it active and deactivates zip', async ({ mount }) => {
  const component = await mount(ArchiveOptionsWrapper, {
    props: { initialOptions: { output_format: 'zip', archive_compression: 5 } },
  })
  const zipBtn = component.getByRole('button', { name: 'zip' })
  const gzBtn  = component.getByRole('button', { name: 'gz' })
  await gzBtn.click()
  await expect(gzBtn).toHaveClass(/seg-active/)
  await expect(zipBtn).not.toHaveClass(/seg-active/)
})
