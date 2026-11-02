const load2 = () => import('./route2.mjs');

globalThis.run = async () => {
  const page = await import('./page.mjs');
  page.read();
  globalThis.route2 = await load2();
  const { value } = await import('./route0.mjs');
  return value;
};
