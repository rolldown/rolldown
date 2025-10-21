import {
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { join } from 'node:path';
import { rolldown, RolldownPlugin } from 'rolldown';
import { expect, test } from 'vitest';

const root = import.meta.dirname;
const input = join(root, 'index.ts');

test('clean outdir', async () => {
  const outdir = join(root, 'dist/clean-dir');
  if (existsSync(outdir)) rmSync(outdir, { recursive: true });

  const bundler = await rolldown({ input, cwd: root });
  await bundler.write({
    dir: outdir,
    entryFileNames: 'index1.js',
    cleanDir: true,
  });
  expect(existsSync(join(outdir, 'index1.js'))).toBe(true);

  await bundler.write({
    dir: outdir,
    entryFileNames: 'index2.js',
    cleanDir: false,
  });
  expect(existsSync(join(outdir, 'index1.js'))).toBe(true);
  expect(existsSync(join(outdir, 'index2.js'))).toBe(true);

  await bundler.write({
    dir: outdir,
    entryFileNames: 'index3.js',
    cleanDir: true,
  });
  expect(existsSync(join(outdir, 'index1.js'))).toBe(false);
  expect(existsSync(join(outdir, 'index2.js'))).toBe(false);
  expect(existsSync(join(outdir, 'index3.js'))).toBe(true);

  rmSync(outdir, { recursive: true });
});

// When cleanDir is true, and there are file output in
// the `generateBundle` hook, the file should not be cleaned.
test('clean outdir hooks', async () => {
  const generateBundleFile = 'generate-bundle.md';
  const generateBundleContent = 'Generate bundle output.';

  const writeBundleFile = 'write-bundle.md';
  const writeBundleContent = 'Write bundle output.';

  function examplePlugin(): RolldownPlugin {
    return {
      name: 'example-plugin',
      generateBundle(outputOptions) {
        const outputDir = outputOptions.dir;
        if (!outputDir) throw new Error('cannot get outdir in plugin');
        mkdirSync(outputDir, { recursive: true });
        writeFileSync(
          join(outputDir, generateBundleFile),
          generateBundleContent,
        );
      },
      writeBundle(outputOptions) {
        const outputDir = outputOptions.dir;
        if (!outputDir) throw new Error('cannot get outdir in plugin');
        mkdirSync(outputDir, { recursive: true });
        writeFileSync(join(outputDir, writeBundleFile), writeBundleContent);
      },
    };
  }

  const outdir = join(root, 'dist/clean-dir-hooks');
  if (existsSync(outdir)) rmSync(outdir, { recursive: true });
  expect(existsSync(outdir)).toBe(false);

  const bundler = await rolldown({
    plugins: [examplePlugin()],
    input,
    cwd: root,
  });
  await bundler.write({
    dir: outdir,
    entryFileNames: 'index.js',
    cleanDir: true,
  });
  expect(existsSync(join(outdir, generateBundleFile))).toBe(false);
  expect(existsSync(join(outdir, writeBundleFile))).toBe(true);
  expect(readFileSync(join(outdir, writeBundleFile), 'utf-8')).toBe(
    writeBundleContent,
  );

  rmSync(outdir, { recursive: true });
});
