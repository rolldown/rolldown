import { helper } from './m.js';
export { q } from './w.js';

(globalThis.entryLog ??= []).push(`entry:${helper()}`);
globalThis.loadD = () => import('./d.js');
