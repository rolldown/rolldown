import type { BindingOutputs } from '../binding';
import { transformToRollupOutput } from '../utils/transform-to-rollup-output';
import type { RolldownOutput } from './rolldown-output';

export class RolldownOutputImpl implements RolldownOutput {
  constructor(private bindingOutputs: BindingOutputs) {
  }

  get output(): RolldownOutput['output'] {
    return transformToRollupOutput(this.bindingOutputs).output;
  }
}
