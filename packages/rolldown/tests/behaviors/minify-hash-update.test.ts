import { join } from 'node:path';
import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

const root = import.meta.dirname;

test('output hash changes when minify options change', async () => {
  const bundle = await rolldown({
    input: join(root, 'minify-hash-update/main.js'),
    cwd: root,
  });

  const outputDisableDropConsole = await bundle.generate({
    entryFileNames: '[name]-[hash].js',
    minify: {
      compress: {
        dropConsole: false,
      },
    },
  });

  const outputEnableDropConsole = await bundle.generate({
    entryFileNames: '[name]-[hash].js',
    minify: {
      compress: {
        dropConsole: true,
      },
    },
  });

  expect(outputDisableDropConsole.output.length).toBe(1);
  expect(outputEnableDropConsole.output.length).toBe(1);
  const chunkDisableDropConsole = outputDisableDropConsole.output[0];
  const chunkEnableDropConsole = outputEnableDropConsole.output[0];

  expect(chunkDisableDropConsole.type).toBe('chunk');
  expect(chunkEnableDropConsole.type).toBe('chunk');

  // Verify console.log is present when dropConsole is false
  if (chunkDisableDropConsole.type === 'chunk') {
    expect(chunkDisableDropConsole.code).toContain('console.log');
  }

  // Verify console.log is removed when dropConsole is true
  if (chunkEnableDropConsole.type === 'chunk') {
    expect(chunkEnableDropConsole.code).not.toContain('console.log');
  }

  // Most importantly: verify the hashes are different
  expect(chunkDisableDropConsole.fileName).not.toBe(
    chunkEnableDropConsole.fileName,
  );

  await bundle.close();
});
