import 'ext-async';
import * as hn from './hn.js';
console.log('h', Object.keys(hn).join());
import('./d.js').then(() => console.log('d loaded'));
