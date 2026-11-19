import sheet from 'sheet' with { type: 'css' };
import data from 'data' with { type: 'json' };
import { used } from './lib/index.js';

export { named } from 'named' with { type: 'css' };

console.log(sheet, data, used, import('lazy', { with: { type: 'css' } }));
