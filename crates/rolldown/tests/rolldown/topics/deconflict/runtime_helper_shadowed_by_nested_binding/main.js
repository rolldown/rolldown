// A dynamic import of CommonJS prints as `import('./dep.js').then((m) => __toESM(m.default))`, so a
// parameter named `__toESM` must not capture the runtime helper.
export async function read(__toESM) {
  const dep = await import('./dep.cjs');
  return [dep.default.v, __toESM];
}
