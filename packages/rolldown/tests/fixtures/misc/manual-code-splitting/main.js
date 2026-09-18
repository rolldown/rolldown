import { decode } from './shared-codec.cjs'; // boot: static use of the shared dep only
document.body.textContent = decode('boot');
window.lazySign = () => import('./heavy.js').then((m) => m.sign('tx')); // heavy: dynamic-only
