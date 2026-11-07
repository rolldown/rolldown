import type { BindingOutputs, ExternalMemoryStatus } from '../binding.cjs';
import { lazyProp } from '../decorators/lazy';
import { transformToRollupOutput } from '../utils/transform-to-rollup-output';
import type { ExternalMemoryHandle } from './external-memory-handle';
import type { OutputAssetImpl } from './output-asset-impl';
import type { OutputChunkImpl } from './output-chunk-impl';
import { PlainObjectLike } from './plain-object-like';
import type { RolldownOutput } from './rolldown-output';

export class RolldownOutputImpl
  extends PlainObjectLike
  implements RolldownOutput, ExternalMemoryHandle
{
  declare mangleCache?: RolldownOutput['mangleCache'];
  // Undefined after `releaseBindings()`.
  private bindingOutputs: BindingOutputs | undefined;

  constructor(bindingOutputs: BindingOutputs) {
    super();
    this.bindingOutputs = bindingOutputs;
    if (bindingOutputs.mangleCache !== undefined) {
      this.mangleCache = bindingOutputs.mangleCache;
    }
  }

  @lazyProp
  get output(): RolldownOutput['output'] {
    if (this.bindingOutputs === undefined) {
      throw new Error('This output has already released its binding objects');
    }
    return transformToRollupOutput(this.bindingOutputs).output;
  }

  /**
   * @internal Cache every output field, then drop the binding wrappers, whose
   * methods keep the binding instance that made them alive.
   */
  releaseBindings(): void {
    for (const item of this.output) {
      (item as OutputChunkImpl | OutputAssetImpl).releaseBindings();
    }
    this.bindingOutputs = undefined;
  }

  __rolldown_external_memory_handle__(keepDataAlive?: boolean): ExternalMemoryStatus {
    const outputs = this.output;
    const results = outputs.map((item) => item.__rolldown_external_memory_handle__(keepDataAlive));

    const allFreed = results.every((r) => r.freed);
    if (!allFreed) {
      const reasons = results
        .filter((r) => !r.freed)
        .map((r) => r.reason)
        .filter(Boolean);
      return {
        freed: false,
        reason: `Failed to free ${reasons.length} item(s): ${reasons.join('; ')}`,
      };
    }
    return { freed: true };
  }
}
