import { big } from './big.js';

(globalThis.order ??= []).push('shared');
export const s = 's' + big;
