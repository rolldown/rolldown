import { f0 } from './vendor.js';

console.log(f0(1));

// A local symbol named `then`, exported under a different name: the namespace
// has no `then` key, so it is NOT thenable.
const then = 'not-a-function';
export { then as ready };

export const done = import('./lazy.js').then(({ fn }) => {
  fn();
});
