import { foo } from './shared.js';

(globalThis.sideEffectLog ??= []).push('a');
foo();
