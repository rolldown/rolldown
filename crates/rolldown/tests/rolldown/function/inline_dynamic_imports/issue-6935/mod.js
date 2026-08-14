export function testFunc() {
  return 1;
}

export async function makeAgent() {
  const { Agent } = await import('./lib');
  return new Agent();
}
