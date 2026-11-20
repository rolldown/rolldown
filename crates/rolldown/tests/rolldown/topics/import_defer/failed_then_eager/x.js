// The self import makes `x.js` a tracked module, with the `__esmState` wrapper.
import defer * as self from './x.js';

export const value = 1;
export const readSelf = () => self.value;
throw new Error('X failed');
