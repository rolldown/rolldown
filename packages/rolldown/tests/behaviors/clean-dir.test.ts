import { existsSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

const root = import.meta.dirname

test('clean out dir', async () => {
  const outdir = join(root, 'dist/clean-dir')
  if (existsSync(outdir)) rmSync(outdir, { recursive: true })
  
  const bundler = await rolldown({ input: 'index.ts', cwd: root })
  await bundler.write({ dir: outdir, entryFileNames: 'index1.js', cleanDir: true })
  expect(existsSync(join(outdir, 'index1.js'))).toBe(true)

  await bundler.write({ dir: outdir, entryFileNames: 'index2.js', cleanDir: false })
  expect(existsSync(join(outdir, 'index1.js'))).toBe(true)
  expect(existsSync(join(outdir, 'index2.js'))).toBe(true)

  await bundler.write({ dir: outdir, entryFileNames: 'index3.js', cleanDir: true })
  expect(existsSync(join(outdir, 'index1.js'))).toBe(false)
  expect(existsSync(join(outdir, 'index2.js'))).toBe(false)
  expect(existsSync(join(outdir, 'index3.js'))).toBe(true)

  rmSync(outdir, { recursive: true })
})
