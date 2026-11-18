// Each module uses its top-level `this` while rebinding `exports`, `module` or `module.exports`,
// or from inside a top-level arrow function.
import declared from './declared.cjs';
import assigned from './assigned.cjs';
import replaced from './replaced.cjs';
import nulled from './nulled.cjs';
import fnexports from './fnexports.cjs';
import varmodule from './varmodule.cjs';
import arrow from './arrow.cjs';
import plain from './plain.cjs';
export const all = { declared, assigned, replaced, nulled, fnexports, varmodule, arrow, plain };
