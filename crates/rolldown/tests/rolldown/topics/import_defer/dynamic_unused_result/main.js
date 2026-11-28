// The result of `import.defer()` is not used.
let x = 0;
while (!x) {
  x++;
  import.defer('./empty.js');
}
export { x };
