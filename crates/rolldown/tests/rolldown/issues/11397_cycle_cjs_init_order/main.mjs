globalThis.run = async () => {
  const page = await import('./page.mjs');
  const route = await import('./route.mjs');
  return [page.getRoute(), route];
};
