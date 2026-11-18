for (var x of [1]) {
}
for (var k in { a: 1 }) {
}
for (var { a } of [{ a: 2 }]) {
}
for (var [b = 3] of [[]]) {
}
for (var {
  c: [d = 5],
} of [{ c: [] }]) {
}
{
  for (var nested of [4]) {
  }
}

export { x, k, a, b, d, nested };
