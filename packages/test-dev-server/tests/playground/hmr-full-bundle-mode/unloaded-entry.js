import { decorate } from './facade.js';
import cjsFacade from './cjs-facade.cjs';
import conditionalCjsFacade from './conditional-cjs-facade.cjs';
import conditionalChanged from './conditional-changed.cjs';

console.log(decorate('unloaded entry'));
console.log(cjsFacade.decorate('unloaded CJS entry'));
console.log(conditionalCjsFacade.decorate('unloaded conditional CJS entry'));
console.log(conditionalChanged);
