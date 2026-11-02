import { dep } from './dep.js';

let latest = dep;

export function current() {
  return latest;
}

import.meta.hot.accept('./dep.js', (mod) => {
  latest = mod.dep;
});
