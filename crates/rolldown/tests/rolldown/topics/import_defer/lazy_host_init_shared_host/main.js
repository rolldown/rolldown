export const access = async () => {
  const p = await import('./p.js');
  const q = await import('./q.js');
  const r = await import('./r.js');
  const a = await import('./a.js');
  return [p.v, q.v, r.read(), a.a].join(',');
};
