import type { InputOptions } from '../options/input-options';
import type { OutputOptions } from '../options/output-options';

export interface RolldownOptions extends InputOptions {
  output?: OutputOptions | OutputOptions[];
}
