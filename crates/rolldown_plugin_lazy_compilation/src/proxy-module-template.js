const lazyExports = (async () => {
  // Remove the current module from the runtime's module cache. Removal is what re-arms a
  // cache-gated factory: the lazy chunk re-registers $STABLE_PROXY_MODULE_ID with the
  // real module, and its tail's initModule would otherwise skip the swap.
  __rolldown_runtime__.removeModuleCache($STABLE_PROXY_MODULE_ID);
  // Dev server will intercept this import and serve the actual module code.
  // We send the proxy module ID (with ?rolldown-lazy=1) so the server can mark it as fetched.
  await import(
    /* @vite-ignore */ `/@vite/lazy?id=${encodeURIComponent($PROXY_MODULE_ID)}&clientId=${__rolldown_runtime__.clientId}`
  );
  // Loading the chunk re-registers this proxy id, exposing the real module's
  // initializer as its own `rolldown:exports` promise. Await that promise (don't
  // just hand back the namespace) so an error thrown while the real module
  // initializes rejects `lazyExports` too, surfacing at the consumer's
  // `await import(...)` (catchable) instead of escaping as an unhandled rejection.
  return await __rolldown_runtime__.loadExports($STABLE_PROXY_MODULE_ID)['rolldown:exports'];
})();

export { lazyExports as 'rolldown:exports' };
