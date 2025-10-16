export function placeholder(...values: number[]) {
  return values.reduce((acc, cur) => acc + cur, 0)
}
