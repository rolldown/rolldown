export async function arrow() {
  const read = () => mod.foo;
  const mod = await import('./arrow-lib.js');
  return read();
}
