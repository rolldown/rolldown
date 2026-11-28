export async function hoisted() {
  function read() {
    return mod.foo;
  }
  const mod = await import('./hoisted-lib.js');
  return read();
}
