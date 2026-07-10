// Separate destructures should accumulate used keys (a, then b) and still drop c.
import('./lib.js').then((ns) => {
  const { a } = ns;
  const { b } = ns;
  console.log(a, b);
});
