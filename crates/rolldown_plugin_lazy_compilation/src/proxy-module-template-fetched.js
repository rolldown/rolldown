const lazyExports = (async () => {
  await import($MODULE_ID);
  return __rolldown_runtime__.loadExports($STABLE_MODULE_ID);
})();

export { lazyExports as 'rolldown:exports' };
