import * as marker from 'init-barrel';
import { snapshot } from './common.js';
const { initValue } = marker.api;
export function getReady() {
  return initValue();
}
export const value = snapshot;
