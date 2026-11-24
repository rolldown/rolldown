globalThis.value = 42;
const ns = await import('./target.js');
console.log(ns.x);
