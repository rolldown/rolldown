// Pure re-export barrel (package sideEffects: false) — but it runs a real side effect by
// importing a non-side-effect-free module. That is the transitive side effect at risk.
import '../side-effectful.js';

export { used } from './used.js';
