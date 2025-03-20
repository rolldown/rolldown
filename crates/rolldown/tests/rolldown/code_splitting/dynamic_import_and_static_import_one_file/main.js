import * as fooNamespace from './foo.js';
import('./foo.js').then(console.log);
console.log(fooNamespace);
