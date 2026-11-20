export const access = async () => {
  const b = await import('./b.js');
  const r = await import('./r.js');
  const a = await import('./a.js');
  await import('./y.js');
  return [b.v, r.read(), a.a].join(',');
};
