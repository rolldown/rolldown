import { SharedEnum } from './dependency.js';

var freeExports = typeof exports == 'object' && exports && !exports.nodeType && exports;
var freeModule = freeExports && typeof module == 'object' && module && !module.nodeType && module;

// Reads the imported dependency binding, whose chunk-root name stays `sharedValue` (the CJS entry
// locals below don't reserve chunk-scope names, so the dependency keeps the bare name). The
// `var sharedValue` local shadows it inside the `__commonJS` closure and must be deconflicted.
var eventKind = SharedEnum.EventMatch;

// Second-order case: deconflicting the `sharedValue` local can't simply pick `sharedValue$1` —
// that name is already taken by the sibling local below. The override must skip it and land on
// `sharedValue$2`, which is why it checks every binding in the module (root + nested), not just
// the chunk scope. (The dependency binding itself is NOT renamed here; only the entry locals are.)
var sharedValue = class {};
var sharedValue$1 = class {};

console.log(eventKind, sharedValue, sharedValue$1, freeModule);
