const lazyExports = (async () => {
  // Dev server will intercept this import and serve the actual module code.
  // We send the proxy module ID (with ?rolldown-lazy=1) so the server can mark it as executed.
  await import(
    `/lazy?id=${
      encodeURIComponent($PROXY_MODULE_ID)
    }&clientId=${__rolldown_runtime__.clientId}`
  );
  // After the module code is loaded, we can get its exports from the runtime.
  // The actual module registers with $MODULE_ID (without ?rolldown-lazy=1).
  return __rolldown_runtime__.loadExports($MODULE_ID);
})();

export { lazyExports as 'rolldown:exports' };
