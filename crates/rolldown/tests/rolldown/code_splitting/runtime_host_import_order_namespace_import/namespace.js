import * as marker from 'init-barrel';
import { snapshot } from './common.js';
export function getReady() {
  return marker.initValue();
}
export const value = snapshot;
