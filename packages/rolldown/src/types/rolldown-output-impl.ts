import type { BindingOutputs } from '../binding';
import { transformToRollupOutput } from '../utils/transform-to-rollup-output';
import type { ExternalMemoryHandle } from './external-memory-handle';
import type { RolldownOutput } from './rolldown-output';

export class RolldownOutputImpl
  implements RolldownOutput, ExternalMemoryHandle
{
  constructor(private bindingOutputs: BindingOutputs) {
  }

  get output(): RolldownOutput['output'] {
    return transformToRollupOutput(this.bindingOutputs).output;
  }

  __rolldown_external_memory_handle__(): boolean {
    let allFreed = true;
    for (const chunk of this.bindingOutputs.chunks) {
      allFreed = chunk.dropInner() && allFreed;
    }
    for (const asset of this.bindingOutputs.assets) {
      allFreed = asset.dropInner() && allFreed;
    }
    return allFreed;
  }
}
