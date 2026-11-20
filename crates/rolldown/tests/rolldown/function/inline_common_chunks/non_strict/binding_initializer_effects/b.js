import { read } from './helper.js';
import './after.js';
globalThis.events.push('B ' + read());
