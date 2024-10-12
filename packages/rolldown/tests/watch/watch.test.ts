import { expect, test } from 'vitest'
import { watch } from 'rolldown'
import fs from 'node:fs'
import path from 'node:path'

test('watch', async () => {
  const input = path.join(import.meta.dirname, './main.js')
  const inputSource = fs.readFileSync(input, 'utf-8')
  const output = path.join(import.meta.dirname, './dist/main.js')
  watch({
    input,
    cwd: import.meta.dirname,
  })
  // sleep 50ms
  await new Promise((resolve) => {
    setTimeout(resolve, 50)
  })
  expect(fs.readFileSync(output, 'utf-8').includes('console.log(1)')).toBe(true)

  // edit file
  fs.writeFileSync(input, 'console.log(2)')
  // sleep 50ms
  await new Promise((resolve) => {
    setTimeout(resolve, 50)
  })
  expect(fs.readFileSync(output, 'utf-8').includes('console.log(2)')).toBe(true)

  // revert change
  fs.writeFileSync(input, inputSource)
})
