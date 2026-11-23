import { f } from './owner.js';
try {
  f();
} catch {}
if (!globalThis.ready) f?.();
