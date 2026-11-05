import './inner.js';
import { value } from 'ext-lazy';
import { join } from 'node:path';
import 'ext-eager';

export const x = [value, join];
