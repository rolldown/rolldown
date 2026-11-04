import { build } from 'rolldown';
import { expect, test } from 'vitest';

// `order` sets which consumer finishes loading first. That changes module indices
// but not the graph or its execution order.
async function bundle(order: string[]) {
  const names = ['a', 'b', 'c', 'd'];
  const files: Record<string, string> = {
    'entry.js':
      names.map((n) => `import { ${n} } from './consumer-${n}.js';`).join('\n') +
      `\nglobalThis.result = [${names.join(', ')}];`,
    'shared.cjs': names.map((n, i) => `exports.${n} = ${i};`).join('\n'),
  };
  for (const name of names) {
    files[`${name}.js`] = `export { ${name} } from './shared.cjs';`;
    files[`consumer-${name}.js`] =
      `import { ${name} as value } from './${name}.js'; export const ${name} = value;`;
  }
  const releases = new Map<string, () => void>();
  const parsed: string[] = [];
  const result = await build({
    input: 'entry.js',
    write: false,
    output: { strictExecutionOrder: true },
    plugins: [
      {
        name: 'controlled-load-order',
        resolveId(source) {
          const file = source.replace('./', '');
          if (file in files) return file;
        },
        async load(id) {
          if (!(id in files)) return;
          if (id.startsWith('consumer-')) {
            await new Promise<void>((resolve) => {
              releases.set(id[9], resolve);
              if (releases.size === order.length) releases.get(order[0])!();
            });
          }
          return { code: files[id], moduleSideEffects: false };
        },
        moduleParsed(info) {
          if (/^[a-d]\.js$/.test(info.id)) {
            parsed.push(info.id[0]);
            releases.get(order[parsed.length])?.();
          }
        },
      },
    ],
  });
  expect(parsed).toEqual(order);
  return result.output[0].code;
}

test('CJS re-export bindings get the same names whichever consumer loads first', async () => {
  const forward = ['a', 'b', 'c', 'd'];
  expect(await bundle([...forward].reverse())).toBe(await bundle(forward));
});
