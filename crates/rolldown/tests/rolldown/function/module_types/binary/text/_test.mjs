import buffer from './dist/main.js';
import assert from 'node:assert';
import fs from 'node:fs/promises';
import path from 'node:path';

assert((await fs.readFile(path.resolve(import.meta.dirname, 'text.data'))).equals(buffer));
