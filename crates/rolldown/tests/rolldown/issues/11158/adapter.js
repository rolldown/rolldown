export async function load() {
  const { lib } = await import('./lib.js');
  return lib();
}
