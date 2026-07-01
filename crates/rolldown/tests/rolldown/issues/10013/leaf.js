import { first, second } from 'dep';

// The entries only pull the re-exported `other`, never `value`, so `dep` stays
// deferred. Under `strictExecutionOrder` the unused `value = first(second)` is
// still kept as a side-effect, but its `import ... from "dep"` gets dropped —
// leaving `first`/`second` as free identifiers in the shared chunk (#10013).
export { other } from './helper.js';

export const value = first(second);
