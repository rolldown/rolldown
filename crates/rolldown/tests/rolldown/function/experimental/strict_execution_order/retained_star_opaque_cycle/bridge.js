export function x() {
  return 1;
}

export { commonBridge as common, underscoreBridge as _ } from './middle.js';
export { common as commonLeaf, _ as underscoreLeaf } from './common.js';
