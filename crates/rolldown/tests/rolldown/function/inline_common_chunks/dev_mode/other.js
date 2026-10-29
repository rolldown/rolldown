import { shared } from './shared.js';

shared();

import.meta.hot.accept('./shared.js', () => {});
