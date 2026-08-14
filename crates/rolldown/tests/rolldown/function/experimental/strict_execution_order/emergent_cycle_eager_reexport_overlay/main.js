// Regression pin for a retained eager re-export barrel next to a potential cross-chunk cycle.
//
// In on-demand mode the consumer-local resolver gives the entry's live `pv` obligation directly
// to `definer`. The forwarder's non-empty retained-path overlay must not also reference
// `init_definer`: doing so would add a phantom A -> B edge which, together with chunk B's eager
// import of chunk A's CJS carrier, would manufacture an A <-> B cycle. In wrap-all mode the
// conservative wrapper path may still contain that edge, but it must defer the carrier read so
// both modes produce the same initialized values.
//
// Source order pins the expected evaluation: a-first (A), the eager carrier reader (B), e-first
// (entry chunk), then the definer subtree (B). The entry-chunk-hosted e-first runs after the
// grouped chunks in the predicted order but before the definer subtree in source order, so
// `definer` deviates (premature) and joins the wrap plan. In on-demand mode `eagerhaz` and the pure
// forwarder stay eager; wrap-all conservatively wraps them.
import './a/a-first.js';
import './b/eagerhaz.js';
import './e-first.js';
import { marker, pv } from './a/forwarder.js';

globalThis.__result = { pv, marker: marker(), carried: globalThis.__carried };
