// `import defer` wraps this module, so the declaration of its deferred namespace object is inside
// the wrapper.
import defer * as ns from './cjs.js';

export const get = () => ns;
