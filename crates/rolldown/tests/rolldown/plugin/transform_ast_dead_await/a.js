import { f } from './main.js';
import './pause.js';

f();
(globalThis.calls ??= []).push('a done');
