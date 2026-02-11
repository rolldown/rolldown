// A leaf module with no dependencies. It is imported by both main.js and app.js,
// but splitting it into a vendor chunk does NOT create a circular dependency
// because the import direction is one-way (main/app -> utils, never utils -> main/app).
export function add(a, b) {
  return a + b;
}

export function multiply(a, b) {
  return a * b;
}
