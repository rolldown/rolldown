import { load } from './loader.js';
(globalThis.log ??= []).push('a');
export const targetPromise = load();
