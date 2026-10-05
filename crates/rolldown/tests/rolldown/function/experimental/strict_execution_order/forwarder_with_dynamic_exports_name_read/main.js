// `sep` is a name only the external module provides. Reading it needs the namespace object of
// `utilities/index.js`, so that module is not transparent and the entry calls its `init_*`.
import { hasExpectedValue, isEqual, sep } from './utilities/index.js';

globalThis.__result = [hasExpectedValue('expected'), isEqual({ key: 1 }, 'key'), typeof sep];
