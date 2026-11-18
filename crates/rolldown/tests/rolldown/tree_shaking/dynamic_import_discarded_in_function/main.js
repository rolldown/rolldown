globalThis.aLoaded = new Promise((resolve) => {
  globalThis.resolveA = resolve;
});
globalThis.dThen = new Promise((resolve) => {
  globalThis.resolveD = resolve;
});

// The value of a discarded `import()` is never read, so `a.js` keeps only its side effects.
function loadA() {
  import('./a.js');
}
loadA();

// An arrow function with an expression body returns the namespace.
export const loadB = () => import('./b.js');

// The usage of `c.js` combines a discarded import and a returned import, so it stays complete.
function preloadC() {
  import('./c.js');
}
preloadC();
export const loadC = () => import('./c.js');

// Promise resolution calls the `then` export of `d.js`, so `d.js` keeps its whole namespace.
function loadD() {
  import('./d.js');
}
loadD();
