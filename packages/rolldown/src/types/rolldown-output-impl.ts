import type { BindingOutputs, ExternalMemoryStatus } from '../binding.cjs';
import { lazyProp } from '../decorators/lazy';
import { transformToRollupOutput } from '../utils/transform-to-rollup-output';
import type { ExternalMemoryHandle } from './external-memory-handle';
import { PlainObjectLike } from './plain-object-like';
import type { RolldownOutput } from './rolldown-output';

export class RolldownOutputImpl extends PlainObjectLike
  implements RolldownOutput, ExternalMemoryHandle
{
  constructor(private bindingOutputs: BindingOutputs) {
    super();
  }

  @lazyProp
  get output(): RolldownOutput['output'] {
    return transformToRollupOutput(this.bindingOutputs).output;
  }

  __rolldown_external_memory_handle__(
    keepDataAlive?: boolean,
  ): ExternalMemoryStatus {
    const outputs = this.output;
    const results = outputs.map((item) =>
      item.__rolldown_external_memory_handle__(keepDataAlive)
    );

    const allFreed = results.every((r) => r.freed);
    if (!allFreed) {
      const reasons = results
        .filter((r) => !r.freed)
        .map((r) => r.reason)
        .filter(Boolean);
      return {
        freed: false,
        reason: `Failed to free ${reasons.length} item(s): ${
          reasons.join('; ')
        }`,
      };
    }
    return { freed: true };
  }
}
