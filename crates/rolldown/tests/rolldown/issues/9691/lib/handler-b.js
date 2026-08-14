// Mirrors @smithy/node-http-handler's node-http2-handler.js. Its only export
// (HandlerB) is unused, but a top-level destructuring read of an imported binding
// is classified as a side effect and kept once the module is force-included.
import { dep_b } from './dep-b.js';

// Top-level read of the imported binding. lazyBarrel drops the `dep-b.js` import
// (handler-b's exports are never requested), but this statement survives — so at
// runtime `dep_b` is undefined.
const { constants } = dep_b;

export class HandlerB {
  constants = constants;
}
