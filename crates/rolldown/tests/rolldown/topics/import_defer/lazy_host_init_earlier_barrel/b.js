import { foo } from './lib/barrel.js';
import './a.js';
import './y.js';

globalThis.__initLog.push('b:' + foo);
