// Nullish coalescing was introduced in ES2020
// With target es2015, this should not be converted to ??
let x;
null == x && (x = true);
export { x };
