// Destructuring a dynamic-import namespace bound to a variable should tree-shake
// unused exports even when the binding can't be inlined (used more than once).
const ns = await import('./lib.js');
const { foo } = ns;
const { foo: foo2 } = ns;
console.log(foo, foo2);
