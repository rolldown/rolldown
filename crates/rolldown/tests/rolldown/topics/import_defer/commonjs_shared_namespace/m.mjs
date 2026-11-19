// A `.mjs` importer uses the Node.js interop mode: `default` is `module.exports`, also when the
// module sets `__esModule`. Thus it cannot share the namespace of the `.js` importers.
import defer * as ns from './x.cjs';

export const getM = () => ns;
