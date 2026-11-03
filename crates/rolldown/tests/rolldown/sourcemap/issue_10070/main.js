import { hello } from './dep.cjs';
import { app } from './app.js';
globalThis.h = hello;
globalThis.a = app;
