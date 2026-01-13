export const conflict = 1

export function getA(x) {
  let conflict$1 = x + 1
  function inner() {
    return conflict$1
  }
  return conflict + inner() - x
}