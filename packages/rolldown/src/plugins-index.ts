// This entry loads the binding (builtin plugin constructors); every such
// entry registers the CurrentThread timer host at import (see timer-host.ts).
import './timer-host';

export { esmExternalRequirePlugin } from './builtin-plugin/constructors';
export { replacePlugin } from './builtin-plugin/replace-plugin';
