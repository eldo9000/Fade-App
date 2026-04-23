import { test, expect } from '@playwright/experimental-ct-svelte'
import DataOptions from '../../src/lib/DataOptions.svelte'

test('DataOptions mounts and renders', async ({ mount }) => {
  const component = await mount(DataOptions, {
    props: {
      options: {
        output_format: 'csv',
        csv_delimiter: ',',
        json_pretty: false,
      } as any,
    },
  })
  await expect(component).toBeVisible()
})
