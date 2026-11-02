export async function b() {
  const { log } = await import('./logger.js');
  return log('b');
}
