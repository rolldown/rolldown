import { RolldownOutputImpl } from './output-impl.js';
import { bindingifyInputOptions } from './input-options.js';

export function build(opts) {
  bindingifyInputOptions(opts);
  return new RolldownOutputImpl();
}
