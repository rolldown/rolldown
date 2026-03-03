// Nullish coalescing was introduced in ES2020
// With target es2015, this should not be converted to ??
let x;
null == x && (x = true);

// Optional chaining was introduced in ES2020
// This should not be transformed either
let y = { a: { b: 1 } };
let z = y && y.a && y.a.b;

export { x, z };
