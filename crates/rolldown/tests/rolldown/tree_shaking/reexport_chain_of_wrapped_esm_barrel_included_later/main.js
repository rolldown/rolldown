import { count, create } from './index.js';

if (count(create()) !== 2) {
  throw new Error('unexpected result');
}

import('./dyn.js').then((m) => {
  if (m.default !== 'tag') {
    throw new Error('unexpected tag');
  }
});
