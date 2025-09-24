import nodeFs from 'node:fs';

export let value = 'edited-foo';

import.meta.hot.accept();
nodeFs.writeFileSync('./ok-0', '');
