import { value } from './value.js';

document.getElementById('late-lazy-chunk-lib').textContent = `lib-v1:${value}`;

import.meta.hot?.accept();
