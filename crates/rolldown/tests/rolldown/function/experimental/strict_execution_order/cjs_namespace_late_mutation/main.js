import * as target from './target.cjs';
import './augment.cjs';

export const result = {
  existing: target.existing,
  addedType: typeof target.added,
  defaultAddedType: typeof target.default?.added,
};
