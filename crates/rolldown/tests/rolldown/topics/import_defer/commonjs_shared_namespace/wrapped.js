// `import defer` wraps this module, so its deferred namespace object stays in the wrapper.
import defer * as ns from './x.cjs';

export const getW = () => ns;
