import { used } from './pkg-barrel/index.js';

// The barrel here is NOT sideEffects:false, so its `[side-effectful] ran` side effect is a real
// (non-waived) one that must survive in both the current model and the obligation rewrite.
console.log('used =', used);
