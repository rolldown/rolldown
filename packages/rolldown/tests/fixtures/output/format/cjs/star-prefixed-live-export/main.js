let count = 0;
export { count as '*count' };

export function increment() {
  count++;
}
