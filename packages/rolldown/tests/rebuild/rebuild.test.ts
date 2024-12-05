import { test, expect, vi, onTestFinished } from 'vitest'
import { defineConfig } from 'rolldown'
import { rebuild } from 'rolldown/experimental'
import fs from 'node:fs'
import path from 'node:path'

test('basic', async () => {
  const dir = path.join(import.meta.dirname, 'fixtures/basic')
  const renderChunkFn = vi.fn()

  const config = defineConfig({
    cwd: dir,
    input: './main.js',
    plugins: [
      {
        name: 'test',
        renderChunk(code, chunk) {
          renderChunkFn(chunk.name, code)
        },
      },
    ],
  })

  // initial build
  const bundle = await rebuild(config)
  const output1 = await bundle.build()
  expect(output1.output.map((c) => c.name)).toMatchInlineSnapshot(`
    [
      "main",
    ]
  `)
  expect(renderChunkFn.mock.calls).toEqual([
    ['main', expect.stringContaining('[dep]')],
  ])
  renderChunkFn.mockClear()

  // edit dep.js
  const file = path.join(dir, 'dep.js')
  const content = fs.readFileSync(file, 'utf-8')
  fs.writeFileSync(file, content.replace('[dep]', '[dep-edit]'))
  onTestFinished(() => fs.writeFileSync(file, content))

  // rebuild
  const output2 = await bundle.build()
  expect(output2.output.map((c) => c.name)).toMatchInlineSnapshot(`
    [
      "main",
      "hmr-update",
    ]
  `)
  expect(renderChunkFn.mock.calls).toEqual([
    ['main', expect.stringContaining('[dep-edit]')],
    ['hmr-update', expect.stringContaining('[dep-edit]')],
  ])
})
