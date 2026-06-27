import type { Plugin } from 'rolldown';
import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

// oxlint-disable-next-line no-control-regex
const removeAnsiColors = (str: string) => str.replace(/\x1b\[[0-9;]*m/g, '');

const virtual = (files: Record<string, string>): Plugin => ({
  name: 'virtual',
  resolveId(id) {
    const key = id.replace(/^\.\//, '');
    if (key in files) return key;
  },
  load(id) {
    return files[id];
  },
});

// #9994: an emitted chunk whose module resolves outside the input base produces a relative `../`
// `[name]`, which rolldown rejects with INVALID_OPTION. The error used to expose only the offending
// string, so users could not tell which module/plugin caused it. It should now name the source
// module both in the message and via `error.id`/`error.ids`.
test('invalid [name] substitution reports the offending chunk module (#9994)', async () => {
  let caught: any;
  try {
    const bundle = await rolldown({
      input: 'entry.js',
      plugins: [
        virtual({
          'entry.js': `export const x = 1`,
          'extra.js': `export const y = 2`,
        }),
        {
          name: 'emit-chunk',
          buildStart() {
            this.emitFile({
              type: 'chunk',
              id: 'extra.js',
              name: '../node_modules/some-dep/index.mjs_entry_Foo',
            });
          },
        },
      ],
    });
    await bundle.generate({ dir: 'dist' });
  } catch (e) {
    caught = e;
  }

  expect(caught, 'build should fail with an INVALID_OPTION error').toBeDefined();
  const err = caught.errors?.[0] ?? caught;
  const message = removeAnsiColors(err.message);

  // The original substitution error is preserved...
  expect(err.code).toBe('INVALID_OPTION');
  expect(message).toContain('Invalid substitution "../node_modules/some-dep/index.mjs_entry_Foo"');
  // ...and is now enriched with the source module + an actionable hint.
  expect(message).toContain('derived from module: extra.js');
  expect(message).toContain('this.emitFile');

  // Programmatic locators that were previously `undefined`.
  expect(err.id).toBe('extra.js');
  expect(err.ids).toContain('extra.js');
});
