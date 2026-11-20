import { execFileSync } from 'node:child_process';

for (const order of [
  [0, 1],
  [1, 0],
]) {
  execFileSync(
    process.execPath,
    [
      '--input-type=module',
      '-e',
      `import assert from 'node:assert/strict';
      await import(${JSON.stringify(new URL('./dist/main.js', import.meta.url).href)});
      for (const index of ${JSON.stringify(order)}) {
        const value = await globalThis.load[index]();
        if (index === 0) assert.deepEqual(value, ['layers', 'layers', 'layers', 'layers']);
        else assert.equal(value.TITLE, 'LAYERS');
      }`,
    ],
    { encoding: 'utf8' },
  );
}
