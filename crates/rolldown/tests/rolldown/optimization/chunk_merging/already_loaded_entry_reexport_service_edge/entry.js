import { a } from './x.js';
export { q } from './w.js';

(globalThis.entryLog ??= []).push(`entry:${a}`);
globalThis.loadD = () => import('./d.js');
