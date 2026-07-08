// FIRST import: this worker env loads the binding, so it must register its
// own timer host (the per-env contract from timer-host.ts). On native the
// process-global driver registry can mask a missing registration (main's
// driver serves), but on the wasm artifacts the registry is per-instance --
// without this, a parallel-plugin worker's instance is genuinely driverless
// and a CurrentThread sleep there panics (Codex task-7 round 4, finding 3).
import './timer-host';
import { parentPort, workerData } from 'node:worker_threads';
import { registerPlugins } from './binding.cjs';
import type { InputOptions } from './options/input-options';
import type { OutputOptions } from './options/output-options';
import type { defineParallelPluginImplementation } from './parallel-plugin';
import { bindingifyPlugin } from './plugin/bindingify-plugin';
import { PluginContextData } from './plugin/plugin-context-data';
import type { WorkerData } from './utils/initialize-parallel-plugins';

const controlPort = parentPort!;
const { registryId, pluginInfos, threadNumber, watchMode } = workerData as WorkerData;
// Plugin callbacks are weak TSFNs and therefore do not keep this worker env
// alive. The owner explicitly terminates workers through `stopWorkers()`;
// retain the control port until that lifecycle boundary. The owner also keeps
// error/exit supervision installed after bootstrap so delayed transport faults
// become retryable close errors instead of uncaught parent-process failures.
controlPort.ref();
(async () => {
  try {
    const plugins = await Promise.all(
      pluginInfos.map(async (pluginInfo) => {
        const pluginModule = await import(pluginInfo.fileUrl);
        const definePluginImpl = pluginModule.default as ReturnType<
          typeof defineParallelPluginImplementation
        >;
        const plugin = await definePluginImpl(pluginInfo.options, {
          threadNumber,
        });
        return {
          index: pluginInfo.index,
          // TODO(sapphi-red): support inputOptions and outputOptions
          plugin: bindingifyPlugin(
            plugin,
            {} as InputOptions,
            {} as OutputOptions,
            // TODO need to find a way to share pluginContextData
            new PluginContextData(() => {}, {} as OutputOptions, [], []),
            [],
            () => {},
            'info' as const,
            watchMode,
          ),
        };
      }),
    );

    registerPlugins(registryId, plugins);

    postBootstrapResult({ type: 'success' });
  } catch (error) {
    postBootstrapResult({
      type: 'error',
      error: createCloneableBootstrapDiagnostic(error),
    });
  }
})();

function postBootstrapResult(message: { type: 'success' } | { type: 'error'; error: Error }): void {
  try {
    controlPort.postMessage(message);
  } catch (postMessageError) {
    // A plugin can throw a value that structured clone cannot transport, and
    // hostile bootstrap code can also disrupt the control port itself. Never
    // leave the explicit ref alive after the parent can no longer receive a
    // terminal message. The uncaught microtask gives WorkerSupervisor a
    // cloneable `error` event even under --unhandled-rejections=warn.
    try {
      controlPort.unref();
      controlPort.close();
    } catch {}
    const bootstrapDiagnostic =
      message.type === 'error'
        ? message.error
        : new Error('Parallel-plugin worker could not report successful initialization');
    const reportingDiagnostic = createCloneableBootstrapDiagnostic(
      postMessageError,
      'Parallel-plugin worker could not report its bootstrap result',
    );
    const terminalDiagnostic = new Error(
      `${bootstrapDiagnostic.message}; ${reportingDiagnostic.message}`,
    );
    terminalDiagnostic.name = 'ParallelPluginBootstrapError';
    queueMicrotask(() => {
      throw terminalDiagnostic;
    });
  }
}

function createCloneableBootstrapDiagnostic(
  thrownValue: unknown,
  prefix = 'Parallel-plugin worker initialization failed',
): Error {
  const detail = readThrownValueDetail(thrownValue);
  const diagnostic = new Error(detail ? `${prefix}: ${detail}` : prefix);
  diagnostic.name = 'ParallelPluginBootstrapError';
  try {
    if (thrownValue instanceof Error && typeof thrownValue.stack === 'string') {
      diagnostic.stack = thrownValue.stack;
    }
  } catch {}
  return diagnostic;
}

function readThrownValueDetail(thrownValue: unknown): string | undefined {
  try {
    if (
      thrownValue !== null &&
      (typeof thrownValue === 'object' || typeof thrownValue === 'function') &&
      'message' in thrownValue
    ) {
      const message = thrownValue.message;
      if (typeof message === 'string' && message.length > 0) return message;
    }
  } catch {}
  try {
    const detail = String(thrownValue);
    return detail === '[object Object]' ? undefined : detail;
  } catch {
    return 'a non-coercible thrown value';
  }
}
