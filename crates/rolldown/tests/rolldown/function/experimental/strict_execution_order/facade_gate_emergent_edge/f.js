// Non-included forwarder: nothing consumes `forwarded`, so `f` is dropped from the output. Its
// `import { unused } from './gs/t.js'` still resolves to the interop-wrapped `t`, and the excluded
// re-export hop in `wrapper` forwards `init_t` through it — the cross-chunk edge the pre-lowering
// baseline never records.
import { unused } from './gs/t.js';
export const forwarded = unused;
