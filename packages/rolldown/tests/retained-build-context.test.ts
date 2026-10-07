// Rollup lets a plugin keep a build hook's context and call it later: vite's
// `vite:watch-package-data` binds `this.addWatchFile` in `buildStart` and calls
// it from `resolveId`. Threadless WASI frees hook boxes eagerly, so these cases
// fail if a build-scoped context box is released before the build call settles,
// including at the native cache invalidation that `write()` fires before
// `writeBundle`.
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import nodePath from 'node:path';
import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

const ENTRY_ID = 'virt:entry.js';
const DEP_ID = 'virt:dep.js';
const MODULES: Record<string, string> = {
  [ENTRY_ID]: `import { value } from '${DEP_ID}';\nconsole.log(value);\n`,
  [DEP_ID]: 'export const value = 42;\n',
};

test('a `buildStart`-bound plugin context still works from `resolveId`', async () => {
  let boundAddWatchFile: ((id: string) => void) | undefined;
  const watched: string[] = [];
  let lateCallError: unknown;

  const bundle = await rolldown({
    input: ENTRY_ID,
    plugins: [
      {
        name: 'retained-build-context',
        buildStart() {
          boundAddWatchFile = this.addWatchFile.bind(this);
        },
        resolveId(id) {
          if (!(id in MODULES)) return;
          try {
            boundAddWatchFile!(`${id}.watched`);
            watched.push(id);
          } catch (error) {
            lateCallError = error;
          }
          return id;
        },
        load: (id) => MODULES[id],
      },
    ],
  });
  const { output } = await bundle.generate({ format: 'esm' });
  await bundle.close();

  expect(lateCallError).toBeUndefined();
  // Guards the premise: a resolveId that never ran would pass vacuously.
  expect(watched).toEqual([ENTRY_ID, DEP_ID]);
  expect(output[0].code).toContain('42');
});

// Read `watchFiles` before `close()`: closing clears the native set.
for (const bindingHook of ['buildStart', 'buildEnd'] as const) {
  test(`a \`${bindingHook}\`-bound plugin context still works from \`writeBundle\` under \`write()\``, async () => {
    let boundAddWatchFile: ((id: string) => void) | undefined;
    let lateCallError: unknown;
    let writeBundleRan = false;
    const watchedId = `virt:${bindingHook}.watched`;
    function bindAddWatchFile(this: { addWatchFile: (id: string) => void }) {
      boundAddWatchFile = this.addWatchFile.bind(this);
    }

    const outDir = await mkdtemp(nodePath.join(tmpdir(), 'rolldown-retained-build-context-'));
    try {
      const bundle = await rolldown({
        input: ENTRY_ID,
        plugins: [
          {
            name: 'retained-build-context',
            buildStart: bindingHook === 'buildStart' ? bindAddWatchFile : undefined,
            buildEnd: bindingHook === 'buildEnd' ? bindAddWatchFile : undefined,
            resolveId: (id) => (id in MODULES ? id : undefined),
            load: (id) => MODULES[id],
            writeBundle() {
              writeBundleRan = true;
              try {
                boundAddWatchFile!(watchedId);
              } catch (error) {
                lateCallError = error;
              }
            },
          },
        ],
      });
      const { output } = await bundle.write({ format: 'esm', dir: outDir });
      const watchFiles = await bundle.watchFiles;
      await bundle.close();

      expect(lateCallError).toBeUndefined();
      // Guards the premise: a writeBundle that never ran would pass vacuously.
      expect(writeBundleRan).toBe(true);
      // `addWatchFile` resolves the id against `cwd`, which defaults to `process.cwd()`.
      expect(watchFiles).toContain(nodePath.resolve(watchedId));
      expect(output[0].code).toContain('42');
    } finally {
      await rm(outDir, { recursive: true, force: true });
    }
  });
}
