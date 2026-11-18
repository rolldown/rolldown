import { fileURLToPath } from 'node:url';
import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

// https://github.com/rolldown/rolldown/issues/11417
test.each([
  ['a.js', 'EXTERNAL_MODULES_CANNOT_BE_TRANSFORMED_TO_MODULES'],
  ['b.js', 'INVALID_EXTERNAL_ID'],
])('rejects conflicting resolutions when %s loads last', async (slow, code) => {
  const lib = fileURLToPath(new URL('./external-module-conflict/lib.js', import.meta.url));
  const files: Record<string, string> = {
    'entry.js': `import './a.js'; import './b.js';`,
    'a.js': `import { join } from ${JSON.stringify(lib)}; console.log(join('a', 'x'));`,
    'b.js': `import { join } from 'lib'; console.log(join('b', 'x'));`,
  };
  const bundle = await rolldown({
    input: 'entry.js',
    plugins: [
      {
        name: 'conflicting-resolutions',
        resolveId(id) {
          if (id in files || id.slice(2) in files) return id.replace('./', '');
          if (id === 'lib') return { id: lib, external: 'absolute' };
        },
        async load(id) {
          if (id === slow) await new Promise((resolve) => setTimeout(resolve, 50));
          return files[id];
        },
      },
    ],
  });

  try {
    await expect(bundle.generate({ format: 'esm' })).rejects.toMatchObject({
      errors: [{ code }],
    });
  } finally {
    await bundle.close();
  }
});
