import { start, stop } from './daemon.js';
export function gateway() {
  return start() + stop();
}
