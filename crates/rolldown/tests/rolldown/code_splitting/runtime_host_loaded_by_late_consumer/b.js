export async function b() {
  const { log } = await import('./log.js');
  return log('b');
}
