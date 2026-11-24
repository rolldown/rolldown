import { annotated, empty, listed, rows } from './lib.js';

export const byList = { label: listed(1), rows },
  [first] = [0],
  afterList = empty();
export const byComment = { label: /* @__PURE__ */ annotated(1), rows },
  [second] = [0],
  afterComment = empty();
