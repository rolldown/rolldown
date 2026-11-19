export const access = async () => {
  const b = await import('./b.js');
  const r = await import('./r.js');
  await import('./y.js');
  return [b.value, r.read()];
};
