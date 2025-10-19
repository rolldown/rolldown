// @restart
import nodeFs from 'node:fs';
import './foo';

nodeFs.writeFileSync('./ok-0', '');
