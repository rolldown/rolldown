// Dynamically imports daemon barrel (triggers __exportAll for namespace)
export async function complete() {
  const mod = await import('./daemon.js');
  return mod.start();
}
