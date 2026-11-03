import { f0 } from './vendor.js';

console.log(f0(1));

export const done = import('./lazy.js').then(({ fn }) => {
  fn();
});
