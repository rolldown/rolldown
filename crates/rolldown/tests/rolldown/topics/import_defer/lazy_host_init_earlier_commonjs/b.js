import x from './x.cjs';
import './a.js';
import './y.js';

globalThis.__initLog.push('b:' + x.p);
