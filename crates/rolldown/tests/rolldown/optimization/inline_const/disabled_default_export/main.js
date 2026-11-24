import { noop } from './noop.js';
import label, { load } from './config.js';

noop();
console.log(label, label, label, (await load()).tag);
