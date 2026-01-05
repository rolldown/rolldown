const lazyExports = (async () => {
  const mod = await import($MODULE_ID);
  return mod;
})();

export { lazyExports as 'rolldown:exports' };
