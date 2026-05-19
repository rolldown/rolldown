const lazyExports = (async () => {
  // Remove the cache of the current module from the runtime's module map.
  // This module with key $STABLE_PROXY_MODULE_ID is swapped in the lazy loaded chunk again with the real module.
  delete __rolldown_runtime__.modules[$STABLE_PROXY_MODULE_ID];
  // Dev server will intercept this import and serve the actual module code.
  // We send the proxy module ID (with ?rolldown-lazy=1) so the server can mark it as fetched.
  await import(
    /* @vite-ignore */ `/@vite/lazy?id=${encodeURIComponent($PROXY_MODULE_ID)}&clientId=${__rolldown_runtime__.clientId}`
  );
  // After the module code is loaded, we can get its exports from the runtime.
  // The actual module registers with $STABLE_MODULE_ID (the stable/relative path).
  return __rolldown_runtime__.loadExports($STABLE_MODULE_ID);
})();

export { lazyExports as 'rolldown:exports' };
