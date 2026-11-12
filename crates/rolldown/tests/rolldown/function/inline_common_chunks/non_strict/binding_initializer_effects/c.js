import lib from './lib.cjs';
import { Box } from './box.js';
import './after.js';
globalThis.events.push('C ' + lib.tag + ' ' + new Box('c').name);
