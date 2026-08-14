import { defineTest } from 'rolldown-tests';

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

// Regression test for the lazy-barrel "dropped local import" bug.
//
// `barrel.js` is a side-effect-free barrel that is also an entry: it imports `x`
// from `m.js` for local use (in `useX`) and re-exports `y` from `m.js`.
// `consumer.js` imports ONLY the re-export `y` (a partial request on `barrel`),
// and `splitter.js` makes `m.js` a shared chunk. Lazy barrel used to leave
// `barrel`'s plain import record for `x` unresolved, dropping `x` from the
// output, so `useX()` threw `ReferenceError: x is not defined` at runtime.
//
// The bug is order-sensitive; delaying `barrel.js`'s transform pins the module
// load order so it reproduces deterministically without the fix.
export default defineTest({
  config: {
    input: {
      barrel: './barrel.js',
      splitter: './splitter.js',
      consumer: './consumer.js',
    },
    experimental: { lazyBarrel: true },
    plugins: [
      {
        name: 'side-effect-free-and-delay-barrel',
        async transform(_code, id) {
          if (id.replace(/\\/g, '/').endsWith('/barrel.js')) {
            await sleep(100);
          }
          return { moduleSideEffects: false };
        },
      },
    ],
  },
  afterTest: async () => {
    await import('./_test.mjs');
  },
});
