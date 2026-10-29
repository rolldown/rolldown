import { foo } from './shared.js';

(globalThis.sideEffectLog ??= []).push('b');
foo();
