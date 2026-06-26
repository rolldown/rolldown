// lazy-init-error: this module is lazily imported by `setup.js` and throws while
// initializing. On the first compile of the lazy chunk (the on-demand
// `@vite/lazy` response) the real module is inlined into the proxy's
// eagerly-created `lazyExports` async IIFE, so its init runs synchronously:
//
//   var init_lazy_init_error_1 = createEsmInitializer(
//     "...lazy-init-error.js?rolldown-lazy=1",
//     (id) => { ...
//       const lazyExports = (async () => {
//         await (init_lazy_init_error_0(), Promise.resolve().then(() => loadExports("...lazy-init-error.js")));
//         return __rolldown_runtime__.loadExports("...lazy-init-error.js");
//       })();
//     }, 1);
//
// `init_lazy_init_error_0()` throws synchronously, so this `lazyExports` rejects
// immediately with no handler attached: the error escapes as an unhandled
// promise rejection instead of surfacing at the consumer's `await import(...)`
// try/catch (see setup.js).
throw new Error('boom during lazy init');
