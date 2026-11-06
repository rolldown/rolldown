import './setup.js';
import { readOrder } from './bridge.js';
import * as ui from './ui/index.js';
export const size = ui.SIZE;
export const order = readOrder();
