// Inner barrel. `setup` (local export) uses `store` from a SEPARATE record;
// `helper` is re-exported. When `outer.js` only requests `helper`, `setup` looks
// unused here, so the `./store.js` record is deferred and `store.js` is dropped.
import { store } from './store.js';

export { helper } from './reexported.js';

export function setup() {
  store.value = 'setup-ran';
  return store.value;
}
