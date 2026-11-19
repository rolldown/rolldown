import { marker } from './state.mjs';
export const value = marker;
export function loop() {
  return import('./z.mjs');
}
