import type { BindingOutputs } from '../binding.cjs';
import { lazyProp } from '../decorators/lazy';
import { transformToRollupOutput } from '../utils/transform-to-rollup-output';
import { PlainObjectLike } from './plain-object-like';
import type { RolldownOutput } from './rolldown-output';

export class RolldownOutputImpl extends PlainObjectLike implements RolldownOutput {
  declare mangleCache?: RolldownOutput['mangleCache'];

  constructor(private bindingOutputs: BindingOutputs) {
    super();
    if (bindingOutputs.mangleCache !== undefined) {
      this.mangleCache = bindingOutputs.mangleCache;
    }
  }

  @lazyProp
  get output(): RolldownOutput['output'] {
    return transformToRollupOutput(this.bindingOutputs).output;
  }
}
