import { foo } from './shared.js';

(globalThis.sideEffectLog ??= []).push('c');
foo();
