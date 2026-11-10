export function sum(a, b) {
  return a + b;
}
export const total = sum(40, globalThis.__two ?? 2);
export const label = 'total ';
