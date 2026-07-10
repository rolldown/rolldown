// rest capture: `a` is named, `rest.b` used via the rest binding -> keep a and b, drop c.
import('./lib.js').then((ns) => {
  const { a, ...rest } = ns;
  console.log(a, rest.b);
});
