globalThis.run = async () => {
  await import('./page.js');
  const ns = await import('./target.js');
  // Keep the whole namespace observable, so the facade check of dynamic entries runs.
  globalThis.targetNs = ns;
  return { before: globalThis.before, value: ns.value };
};
