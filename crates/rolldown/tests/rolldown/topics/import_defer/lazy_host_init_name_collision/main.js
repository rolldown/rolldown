export const access = async () => {
  const source = await import('./a.init.js');
  const b = await import('./b.js');
  const r = await import('./r.js');
  const a = await import('./a.js');
  await import('./y.js');
  return [source.tag, b.v, r.read(), a.a].join(',');
};
