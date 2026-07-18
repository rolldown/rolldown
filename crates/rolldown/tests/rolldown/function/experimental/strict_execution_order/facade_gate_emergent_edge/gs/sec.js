// Interop-wrapped entry (CommonJS -> WrapKind::Cjs). Its chunk (group `gs`) co-hosts the
// order-wrapped `t`. The only cross-chunk edge into this chunk is the emergent `A -> gs` hop the
// excluded forwarder projects — there is no baseline edge — so a facade gate reading pre-lowering
// edges leaves `sec`'s inline `require_sec()` trigger in the shared chunk. Reading the post-lowering
// edges instead splits `sec` into a facade, so evaluating the shared chunk never runs its program.
const { tv } = require('./t.js');
globalThis.__secRan = (globalThis.__secRan ?? 0) + 1;
(globalThis.__events ??= []).push('sec-body');
module.exports = { s: tv };
