import { used } from './pkg-barrel/index.js';

// The `[side-effectful] ran` log is reached only transitively through the `sideEffects: false`
// barrel. Under current strict it appears in the bundle and runs before this line; the
// obligation rewrite would prune the barrel and drop it, matching default mode.
console.log('used =', used);
