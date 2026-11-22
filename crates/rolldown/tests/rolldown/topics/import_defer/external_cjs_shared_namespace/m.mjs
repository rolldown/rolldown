// A `.mjs` importer uses the Node.js interop mode, so it gets its own deferred namespace.
import defer * as ns from 'ext-shared';

export const getM = () => ns;
