globalThis.run = async () => {
  await import('./page.mjs');
  const ns = await import('./target.mjs');
  globalThis.saved = ns;
  ns.increment();
  return ns.count;
};
