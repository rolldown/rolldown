import { version } from '../package.json';

import { defineConfig } from './utils/define-config';
import { loadConfig } from './utils/load-config';

export { defineConfig, loadConfig };

export const VERSION: string = version;
