export async function read(x) {
  const { readIt } = await import('./lazy.js');
  return readIt(x);
}
export function create() {
  return 1;
}
