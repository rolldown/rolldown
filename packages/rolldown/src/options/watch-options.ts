import type { InputOptions } from '../options/input-options';
import type { OutputOptions } from '../options/output-options';

export interface WatchOptions extends InputOptions {
  output?: OutputOptions | OutputOptions[];
}
