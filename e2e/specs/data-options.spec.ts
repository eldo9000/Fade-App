import { test, expect } from '@playwright/experimental-ct-svelte'
import DataOptionsWrapper from '../wrappers/DataOptionsWrapper.svelte'

test('DataOptions renders delimiter buttons for csv', async ({ mount }) => {
  const component = await mount(DataOptionsWrapper, {
    props: {
      initialOptions: {
        output_format: 'csv',
        csv_delimiter: ',',
        pretty_print: false,
      },
    },
  })
  await expect(component.getByRole('button', { name: ',' })).toBeVisible()
  await expect(component.getByRole('button', { name: ';' })).toBeVisible()
  await expect(component.getByRole('button', { name: 'Tab' })).toBeVisible()
  await expect(component.getByRole('button', { name: '|' })).toBeVisible()
})

test('DataOptions comma delimiter is active by default', async ({ mount }) => {
  const component = await mount(DataOptionsWrapper, {
    props: {
      initialOptions: {
        output_format: 'csv',
        csv_delimiter: ',',
        pretty_print: false,
      },
    },
  })
  await expect(component.getByRole('button', { name: ',' })).toHaveClass(/seg-active/)
})

test('DataOptions clicking semicolon makes it active', async ({ mount }) => {
  const component = await mount(DataOptionsWrapper, {
    props: {
      initialOptions: {
        output_format: 'csv',
        csv_delimiter: ',',
        pretty_print: false,
      },
    },
  })
  const semicolonBtn = component.getByRole('button', { name: ';' })
  await semicolonBtn.click()
  await expect(semicolonBtn).toHaveClass(/seg-active/)
})

test('DataOptions clicking Tab makes it active and deactivates comma', async ({ mount }) => {
  const component = await mount(DataOptionsWrapper, {
    props: {
      initialOptions: {
        output_format: 'csv',
        csv_delimiter: ',',
        pretty_print: false,
      },
    },
  })
  const commaBtn = component.getByRole('button', { name: ',' })
  const tabBtn   = component.getByRole('button', { name: 'Tab' })
  await tabBtn.click()
  await expect(tabBtn).toHaveClass(/seg-active/)
  await expect(commaBtn).not.toHaveClass(/seg-active/)
})

test('DataOptions clicking pipe makes it active', async ({ mount }) => {
  const component = await mount(DataOptionsWrapper, {
    props: {
      initialOptions: {
        output_format: 'csv',
        csv_delimiter: ',',
        pretty_print: false,
      },
    },
  })
  const pipeBtn = component.getByRole('button', { name: '|' })
  await pipeBtn.click()
  await expect(pipeBtn).toHaveClass(/seg-active/)
})

test('DataOptions json format shows pretty print checkbox', async ({ mount }) => {
  const component = await mount(DataOptionsWrapper, {
    props: {
      initialOptions: {
        output_format: 'json',
        csv_delimiter: ',',
        pretty_print: false,
      },
    },
  })
  await expect(component.locator('text=Pretty print')).toBeVisible()
})

test('DataOptions pretty print checkbox is unchecked by default', async ({ mount }) => {
  const component = await mount(DataOptionsWrapper, {
    props: {
      initialOptions: {
        output_format: 'json',
        csv_delimiter: ',',
        pretty_print: false,
      },
    },
  })
  const checkbox = component.locator('input.fade-check')
  await expect(checkbox).not.toBeChecked()
})

test('DataOptions clicking pretty print checkbox checks it', async ({ mount }) => {
  const component = await mount(DataOptionsWrapper, {
    props: {
      initialOptions: {
        output_format: 'json',
        csv_delimiter: ',',
        pretty_print: false,
      },
    },
  })
  const checkbox = component.locator('input.fade-check')
  await checkbox.click()
  await expect(checkbox).toBeChecked()
})

test('DataOptions clicking pretty print checkbox twice unchecks it', async ({ mount }) => {
  const component = await mount(DataOptionsWrapper, {
    props: {
      initialOptions: {
        output_format: 'json',
        csv_delimiter: ',',
        pretty_print: false,
      },
    },
  })
  const checkbox = component.locator('input.fade-check')
  await checkbox.click()
  await expect(checkbox).toBeChecked()
  await checkbox.click()
  await expect(checkbox).not.toBeChecked()
})
