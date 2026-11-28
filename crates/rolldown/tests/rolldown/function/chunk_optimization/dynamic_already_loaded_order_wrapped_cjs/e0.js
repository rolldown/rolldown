import './t.cjs';
import './b0.js';
import './x.cjs';

globalThis.wrappedCjsLog.push('e0');
export const loadB0 = () => import('./b0.js').then((m) => m.v);
