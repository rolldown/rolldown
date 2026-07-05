import { defineTest } from 'rolldown-tests';

// Regression test for https://github.com/rolldown/rolldown/issues/9713
//
// `outer.js` is a side-effect-free barrel whose SINGLE import record from
// `inner.js` is BOTH used locally (`setup`, called by the local export `build`)
// and re-exported (`helper`) -- a "mixed" record. The entry requests the local
// export `build` and the re-export `helper` together.
//
// Lazy barrel resolved `helper` first (marking that record "occupied"), then the
// `has_local_export` branch skipped merging the locally-used `setup` into the
// request. So `outer.js` asked `inner.js` only for `helper`; `inner.js`'s own
// `store` import (used only by `setup`) was deferred, never re-resolved, and
// `store.js` was dropped from the bundle. The emitted `setup` then referenced an
// undefined `store`, throwing `ReferenceError: store is not defined` at runtime.
export default defineTest({
  config: {
    input: './entry.js',
    experimental: { lazyBarrel: true },
    plugins: [
      {
        name: 'side-effect-free-barrel',
        transform(_code, id) {
          // The entry keeps its side effects; everything else is a
          // side-effect-free barrel so it becomes a lazy-barrel candidate.
          if (id.replace(/\\/g, '/').endsWith('/entry.js')) return null;
          return { moduleSideEffects: false };
        },
      },
    ],
  },
  afterTest: async () => {
    await import('./_test.mjs');
  },
});
