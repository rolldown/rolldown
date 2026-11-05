import * as barrel from './barrel.js';
console.log('h', Object.keys(barrel).join());
import('./d.js').then(() => console.log('d loaded'));
