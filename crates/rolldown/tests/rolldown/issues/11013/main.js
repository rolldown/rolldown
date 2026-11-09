export async function f() {
  const a = 'no-such-pkg-d';
  const existing = './dep.js';
  return [await import(a), await import(existing), await import('no-such-\u0070kg-h')];
}
