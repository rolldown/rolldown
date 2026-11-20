import { execFileSync } from 'node:child_process';

for (const order of [
  [0, 1],
  [1, 0],
]) {
  // Each order needs a fresh module cache to detect reads before NAME is initialized.
  execFileSync(
    process.execPath,
    [
      '--input-type=module',
      '-e',
      `import assert from 'node:assert/strict';
      await import(${JSON.stringify(new URL('./dist/main.js', import.meta.url).href)});
      for (const index of ${JSON.stringify(order)}) {
        const value = await globalThis.load[index]();
        assert.equal(index === 0 ? value : value.TITLE, index === 0 ? 'layers' : 'LAYERS');
      }`,
    ],
    { encoding: 'utf8' },
  );
}
