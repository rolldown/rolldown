export { q } from './w.js';
(globalThis.entryLog ??= []).push('entry runs');
globalThis.loadD = () => import('./d.js');
