import { foo } from './foo.js'
console.log(foo);

import('./foo.js').then(mod => console.log(mod.foo));
