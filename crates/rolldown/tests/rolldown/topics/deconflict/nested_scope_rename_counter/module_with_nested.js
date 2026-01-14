// This module has a function with nested 'a' and 'a$1' parameters.
// When 'a' shadows top-level 'a' from other.js (different module),
// it should be renamed to 'a$2', NOT 'a$1' (which already exists).
export function test(a, a$1) {
  console.log(a, a$1);
  return [a, a$1];
}
