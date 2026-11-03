import { foo } from './shared.js';

// Top-level side effect of entry `a` (an observable global mutation, like the
// `addEventListener` in the original report). Executing entry `b` must NOT run it.
(globalThis.sideEffectLog ??= []).push('a');
foo();
