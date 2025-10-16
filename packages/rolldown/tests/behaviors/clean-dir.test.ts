import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

const root = import.meta.dirname

test('clean out dir', async () => {
  const bundler = await rolldown({ input: 'index.ts', cwd: root })
  const outdir = 'dist'
  await bundler.write({ dir: outdir, entryFileNames: 'index1.js', cleanDir: true })
  expect(existsSync(join(root, 'dist/index1.js'))).toBe(true)

  await bundler.write({ dir: outdir, entryFileNames: 'index2.js', cleanDir: false })
  expect(existsSync(join(root, 'dist/index1.js'))).toBe(true)
  expect(existsSync(join(root, 'dist/index2.js'))).toBe(true)

  await bundler.write({ dir: outdir, entryFileNames: 'index3.js', cleanDir: true })
  expect(existsSync(join(root, 'dist/index1.js'))).toBe(false)
  expect(existsSync(join(root, 'dist/index2.js'))).toBe(false)
  expect(existsSync(join(root, 'dist/index3.js'))).toBe(true)
})
