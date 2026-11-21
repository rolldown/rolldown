import { f } from './main.js';
import './bridge.js';

f();
(globalThis.calls ??= []).push('a done');
