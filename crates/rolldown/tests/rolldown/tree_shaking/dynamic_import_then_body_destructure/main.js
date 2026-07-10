// Destructuring the dynamic-import namespace inside the callback body should
// tree-shake unused exports the same as `ns.foo` member access.
import('./lib.js').then((ns) => {
  const { foo } = ns;
  console.log(foo);
});
