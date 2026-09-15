// Rollup lets a plugin keep the context object a BUILD hook handed it and use
// it after that hook has settled. The shipping instance of this is vite's
// `vite:watch-package-data`, which binds `this.addWatchFile` in `buildStart`
// and calls it much later, from inside `resolveId`, through a `packageCache`
// setter:
//
//   vite/packages/vite/src/node/packages.ts  (`watchPackageDataPlugin`)
//     buildStart() { watchFile = this.addWatchFile.bind(this) }
//
// The threadless-WASI flavor never runs GC finalizers, so every other hook
// wrapper releases its native boxes the moment the invocation settles (see
// `src/utils/threadless-free.ts`). Doing that to the build-scoped plugin
// contexts would make the bound `addWatchFile` above throw "Memory has been
// freed" mid-resolve, so `buildStart`/`buildEnd` park their context box on the
// build-scoped registry instead and it is drained once the `generate()` /
// `write()` call settles (`plugin-context-data.ts` `retainContextBox`).
//
// That settle is the ONLY drain: the native `invalidateJsSideCache` callback
// fires between `generateBundle` and `writeBundle`, so draining there would
// free a `buildStart`-bound context before `writeBundle` could use it. The
// `write()` cases below pin that ordering.
//
// These tests are flavor-independent on purpose: the idiom must work
// everywhere, and on native/threaded-WASI they guard against a future eager
// release being wired up here.
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
          // The late call: `buildStart` has long since settled, so its context
          // box has to still be alive here.
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

// `writeBundle` runs AFTER the native cache invalidation that `write()` fires
// at the end of `bundle_up` (`crates/rolldown/src/bundle/bundle.rs`), so a
// build-hook-bound context reaching it proves the registry survived that
// callback. Read `watchFiles` before `close()`: closing clears the native set.
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
              // The late call: the build hooks settled long ago and the native
              // cache invalidation has already fired, so the context box has to
              // still be alive here.
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
      expect(watchFiles).toContain(watchedId);
      expect(output[0].code).toContain('42');
    } finally {
      await rm(outDir, { recursive: true, force: true });
    }
  });
}
