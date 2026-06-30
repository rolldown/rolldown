// Repro for #9882. The `exports` reference makes rolldown wrap this entry in
// `__commonJSMin((exports, module) => { ... })`. The local `var sharedValue` collides with the
// dependency's chunk-root `sharedValue` binding; pre-fix it hoists to the top of the closure and
// shadows it, so `SharedEnum.EventMatch` reads the inner `undefined` -> TypeError. The local is a
// `class {}` (not a constant) so it survives as a real declaration instead of being inlined away.
import { SharedEnum } from './dependency.js';

exports.kind = SharedEnum.EventMatch;
var sharedValue = class {};
exports.local = sharedValue;
