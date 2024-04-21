export function isEmptySourcemapFiled(
  array: undefined | (string | null)[],
): boolean {
  if (!array) {
    return true
  }
  if (array.length === 0 || !array[0] /* null or '' */) {
    return true
  }
  return false
}
