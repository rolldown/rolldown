export async function complete() {
  const mod = await import('./daemon.js');
  return mod.start();
}
