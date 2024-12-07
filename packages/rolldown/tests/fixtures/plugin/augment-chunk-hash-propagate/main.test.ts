import { test, expect } from 'vitest'
import { rolldown } from 'rolldown'

test('propagate augmentChunkHash to parent chunks', async () => {
  async function runBuild(augment: string) {
    const bundle = await rolldown({
      input: './main.js',
      cwd: import.meta.dirname,
      plugins: [
        {
          name: 'test',
          augmentChunkHash(chunk) {
            if (chunk.name === 'dep2') {
              return augment
            }
          },
        },
      ],
    })
    const result = await bundle.write({
      entryFileNames: '[name]-[hash].js',
    })
    return Object.fromEntries(result.output.map((c) => [c.name, c.fileName]))
  }

  const result1 = await runBuild('1')
  const result2 = await runBuild('2')
  expect.soft(result1['main']).not.toBe(result2['main'])
  expect.soft(result1['dep1']).not.toBe(result2['dep1'])
  expect.soft(result1['dep2']).not.toBe(result2['dep2'])
})
