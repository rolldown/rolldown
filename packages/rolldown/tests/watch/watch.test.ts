import { expect, test, vi } from 'vitest'
import { watch } from 'rolldown'
import fs from 'node:fs'
import path from 'node:path'

test('watch', async () => {
  const input = path.join(import.meta.dirname, './main.js')
  const buildStartFn = vi.fn()
  watch({
    input,
    plugins: [
      {
        buildStart: () => {
          buildStartFn()
        },
      },
    ],
  })
  // sleep 100ms
  await new Promise((resolve) => {
    setTimeout(resolve, 100)
  })
  expect(buildStartFn).toHaveBeenCalledTimes(1)

  // edit file
  fs.writeFileSync(input, 'console.log(1)')
  // sleep 100ms
  await new Promise((resolve) => {
    setTimeout(resolve, 100)
  })
  expect(buildStartFn).toHaveBeenCalledTimes(2)

  // revert change
  fs.writeFileSync(input, '')
})
