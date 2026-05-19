import { sym } from './client-only.js';

export default function entry() {
  return 'server-entry-default ' + String(sym);
}

export const routes = {
  '/': () => import('./route.js'),
};
