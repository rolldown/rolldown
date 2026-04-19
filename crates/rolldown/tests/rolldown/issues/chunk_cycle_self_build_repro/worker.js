import { PluginContextImpl } from './plugin-context.js';

export function setupWorker() {
  return new PluginContextImpl();
}
