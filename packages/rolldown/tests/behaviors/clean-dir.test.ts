import {
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from 'node:fs';
import { join } from 'node:path';
import type { RolldownPlugin } from 'rolldown';
import { rolldown } from 'rolldown';
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

test('clean default outdir', async () => {
  const defaultOutdir = join(root, 'dist');
  if (existsSync(defaultOutdir)) rmSync(defaultOutdir, { recursive: true });

  const bundler = await rolldown({ input, cwd: root });

  // Without setting dir, use default 'dist'
  await bundler.write({
    entryFileNames: 'index1.js',
    cleanDir: true,
  });
  expect(existsSync(join(defaultOutdir, 'index1.js'))).toBe(true);

  await bundler.write({
    entryFileNames: 'index2.js',
    cleanDir: true,
  });
  // index1.js should be cleaned
  expect(existsSync(join(defaultOutdir, 'index1.js'))).toBe(false);
  expect(existsSync(join(defaultOutdir, 'index2.js'))).toBe(true);

  rmSync(defaultOutdir, { recursive: true });
});

test.each(['output directory', 'output directory with trailing separator', 'directory entry'])(
  'clean outdir removes a symlink at the %s without deleting its target contents',
  async (location) => {
    const testDir = join(root, 'dist/clean-dir-symlink');
    const projectDir = join(testDir, 'project');
    const outdir = join(projectDir, 'dist');
    const targetDir = join(testDir, 'target');
    const targetFile = join(targetDir, 'nested/file.txt');
    const content = 'Keep this file.';

    rmSync(testDir, { recursive: true, force: true });
    try {
      mkdirSync(projectDir, { recursive: true });
      mkdirSync(join(targetDir, 'nested'), { recursive: true });
      writeFileSync(targetFile, content);
      if (location === 'directory entry') mkdirSync(outdir, { recursive: true });
      const link = location === 'directory entry' ? join(outdir, 'link') : outdir;
      symlinkSync(targetDir, link, process.platform === 'win32' ? 'junction' : 'dir');

      const bundler = await rolldown({ input, cwd: projectDir });
      await bundler.write({
        dir: location === 'output directory with trailing separator' ? 'dist/' : 'dist',
        entryFileNames: 'index.js',
        cleanDir: true,
      });

      expect(readFileSync(targetFile, 'utf-8')).toBe(content);
      expect(lstatSync(outdir).isSymbolicLink()).toBe(false);
      expect(readdirSync(outdir)).toEqual(['index.js']);
      expect(readdirSync(targetDir)).toEqual(['nested']);
    } finally {
      rmSync(testDir, { recursive: true, force: true });
    }
  },
);

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
        writeFileSync(join(outputDir, generateBundleFile), generateBundleContent);
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

  expect(existsSync(join(outdir, generateBundleFile))).toBe(true);
  expect(readFileSync(join(outdir, generateBundleFile), 'utf-8')).toBe(generateBundleContent);

  expect(existsSync(join(outdir, writeBundleFile))).toBe(true);
  expect(readFileSync(join(outdir, writeBundleFile), 'utf-8')).toBe(writeBundleContent);

  rmSync(outdir, { recursive: true });
});
