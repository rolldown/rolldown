// `a` via destructure, `b` via member access on the same binding -> keep a and b, drop c.
import('./lib.js').then((ns) => {
  const { a } = ns;
  console.log(a, ns.b);
});
