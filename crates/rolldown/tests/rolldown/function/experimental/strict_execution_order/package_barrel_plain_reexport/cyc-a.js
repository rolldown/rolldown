// Half of a two-chunk static import cycle (paired with cyc-b.js). Manual chunk groups place cyc-a
// and cyc-b in separate chunks that import each other, so the predicted chunk-import graph has a
// cycle reachable from this subtree. Under on-demand wrapping that cycle forces every wrap-eligible
// module in the subtree — including the otherwise-eager pure definers — into the wrap plan, so the
// barrel-forward bug reproduces in on-demand mode as well.
import './cyc-b.js';

(globalThis.__events ??= []).push('cyc-a');

export const a = 1;
