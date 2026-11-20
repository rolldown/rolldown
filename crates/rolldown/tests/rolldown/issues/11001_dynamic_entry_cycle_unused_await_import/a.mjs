import { marker } from './state.mjs';
export const value = marker;
export function unused() {
  import('./z.mjs');
}
