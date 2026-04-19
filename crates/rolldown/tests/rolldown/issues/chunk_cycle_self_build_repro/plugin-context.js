import { PlainObjectLike } from './plain-object.js';
import { preserveEntrySignatures } from './input-options.js';

export class PluginContextImpl extends PlainObjectLike {
  preserve(value) {
    return preserveEntrySignatures(value);
  }
}
