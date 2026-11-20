import 'ext1';
import { s } from './shared.js';
import 'ext2';

(globalThis.order ??= []).push('a:' + s);
