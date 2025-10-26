import type { BindingOutputs } from '../binding';
import { lazy } from '../decorators/lazy';
import { nonEnumerable } from '../decorators/non-enumerable';
import { transformToRollupOutput } from '../utils/transform-to-rollup-output';
import type { ExternalMemoryHandle } from './external-memory-handle';
import type { RolldownOutput } from './rolldown-output';

export class RolldownOutputImpl
  implements RolldownOutput, ExternalMemoryHandle
{
  constructor(private bindingOutputs: BindingOutputs) {
  }

  @lazy
  get output(): RolldownOutput['output'] {
    return transformToRollupOutput(this.bindingOutputs).output;
  }

  @nonEnumerable
  __rolldown_external_memory_handle__(keepDataAlive?: boolean): boolean {
    let allFreed = true;
    // Get the output array which contains OutputChunkImpl and OutputAssetImpl instances
    const outputs = this.output;
    for (const item of outputs) {
      // Call each item's __rolldown_external_memory_handle__ to ensure all lazy fields are evaluated when needed
      allFreed = item.__rolldown_external_memory_handle__(keepDataAlive) &&
        allFreed;
    }
    return allFreed;
  }
}
