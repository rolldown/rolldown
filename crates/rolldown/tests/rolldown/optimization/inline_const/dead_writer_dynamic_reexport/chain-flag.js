// `middle.js` has dynamic exports, so `export *` of it uses the namespace object of this module
// with each `treeshake.commonjs` value.
export let chained = false;

export function enable_chained() {
  chained = true;
}

export * from './middle.js';
