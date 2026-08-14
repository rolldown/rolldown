import type { BindingLoadPluginContext, BindingPluginContext } from '../binding.cjs';
import type { LogHandler } from '../log/log-handler';
import type { LogLevelOption } from '../log/logging';
import type { OutputOptions } from '../options/output-options';
import type { Plugin } from './index';
import { PluginContextImpl } from './plugin-context';
import type { PluginContextData } from './plugin-context-data';

export class LoadPluginContextImpl extends PluginContextImpl {
  constructor(
    outputOptions: OutputOptions,
    context: BindingPluginContext,
    plugin: Plugin,
    data: PluginContextData,
    private inner: BindingLoadPluginContext,
    moduleId: string,
    onLog: LogHandler,
    logLevelOption: LogLevelOption,
    watchMode: boolean,
  ) {
    super(outputOptions, context, plugin, data, onLog, logLevelOption, watchMode, moduleId);
  }

  public addWatchFile(id: string): void {
    // Use the inner context's addWatchFile which tracks dependencies for HMR
    this.inner.addWatchFile(id);
  }
}
