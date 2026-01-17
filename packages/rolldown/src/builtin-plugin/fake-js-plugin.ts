import type { BindingFakeJsPluginConfig } from '../binding.cjs';
import { BuiltinPlugin } from './utils';

export function fakeJsPlugin(config?: BindingFakeJsPluginConfig): BuiltinPlugin {
  return new BuiltinPlugin('builtin:fake-js', config);
}
