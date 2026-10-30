import lib from './lib.cjs';
import { Box } from './box.js';
const box = new Box('helper');
export const read = () => lib.tag + ' ' + box.name;
