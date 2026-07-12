import type { BindingBundlerOptions } from '../binding.cjs';
import type { LogHandler } from '../log/log-handler';
import { getLogger, getOnLog } from '../log/logger';
import { LOG_LEVEL_INFO } from '../log/logging';
import type { InputOptions } from '../options/input-options';
import type { OutputOptions } from '../options/output-options';
import { PluginContextData } from '../plugin/plugin-context-data';
import { PluginDriver } from '../plugin/plugin-driver';
import { getObjectPlugins } from '../plugin/plugin-driver';
import { bindingifyInputOptions } from './bindingify-input-options';
import { bindingifyOutputOptions } from './bindingify-output-options';
import { initializeParallelPlugins } from './initialize-parallel-plugins';
import {
  allocateParallelPluginMetricsId,
  captureProcessMetrics,
  createMetricsRuntime,
  metricsStage,
  metricsTimestamp,
  parallelPluginMetricsEnabled,
  validateCreateBundlerOptionsMetrics,
  writeValidatedMetrics,
  type CreateBundlerOptionsMetrics,
  type MetricsStage,
  type PluginBindingMetric,
} from './parallel-plugin-init-metrics';
import {
  ANONYMOUS_OUTPUT_PLUGIN_PREFIX,
  ANONYMOUS_PLUGIN_PREFIX,
  checkOutputPluginOption,
  normalizePluginOption,
  normalizePlugins,
} from './normalize-plugin-option';

export async function createBundlerOptions(
  inputOptions: InputOptions,
  outputOptions: OutputOptions,
  watchMode: boolean,
): Promise<BundlerOptionWithStopWorker> {
  const metricsEnabled = parallelPluginMetricsEnabled();
  const createBundlerOptionsStartedAt = metricsEnabled ? metricsTimestamp() : undefined;
  const stages: Record<string, MetricsStage> = {};
  const pluginBindingMetrics: PluginBindingMetric[] = [];
  const metricsRuntimeSetupStartedAt = createBundlerOptionsStartedAt;
  const metricsRuntime = metricsEnabled ? await createMetricsRuntime() : undefined;
  finishStage(stages, 'metricsRuntimeSetup', metricsRuntimeSetupStartedAt);
  const metricsId = metricsEnabled ? allocateParallelPluginMetricsId() : undefined;
  const afterMetricsRuntimeSetupAtCreateBundlerOptionsStart = metricsRuntime
    ? captureProcessMetrics(metricsRuntime)
    : undefined;

  const normalizeInputStartedAt = metricsEnabled ? metricsTimestamp() : undefined;
  const inputPlugins = await normalizePluginOption(inputOptions.plugins);
  finishStage(stages, 'normalizeInputPluginOption', normalizeInputStartedAt);
  const normalizeOutputStartedAt = metricsEnabled ? metricsTimestamp() : undefined;
  const outputPlugins = await normalizePluginOption(outputOptions.plugins);
  finishStage(stages, 'normalizeOutputPluginOption', normalizeOutputStartedAt);

  const logLevel = inputOptions.logLevel || LOG_LEVEL_INFO;
  const onLog = getLogger(
    getObjectPlugins(inputPlugins),
    getOnLog(inputOptions, logLevel),
    logLevel,
    watchMode,
  );

  // The `outputOptions` hook is called with the input plugins and the output plugins
  const outputOptionsHookStartedAt = metricsEnabled ? metricsTimestamp() : undefined;
  outputOptions = PluginDriver.callOutputOptionsHook(
    [...inputPlugins, ...outputPlugins],
    outputOptions,
    onLog,
    logLevel,
    watchMode,
  );
  finishStage(stages, 'outputOptionsHook', outputOptionsHookStartedAt);

  const normalizeHookOutputStartedAt = metricsEnabled ? metricsTimestamp() : undefined;
  const hookOutputPlugins = await normalizePluginOption(outputOptions.plugins);
  finishStage(stages, 'normalizeHookOutputPluginOption', normalizeHookOutputStartedAt);
  const normalizePluginObjectsStartedAt = metricsEnabled ? metricsTimestamp() : undefined;
  const normalizedInputPlugins = normalizePlugins(inputPlugins, ANONYMOUS_PLUGIN_PREFIX);
  const normalizedOutputPlugins = normalizePlugins(
    hookOutputPlugins,
    ANONYMOUS_OUTPUT_PLUGIN_PREFIX,
  );

  const plugins = [
    ...normalizedInputPlugins,
    ...checkOutputPluginOption(normalizedOutputPlugins, onLog),
  ];
  finishStage(stages, 'normalizePluginObjects', normalizePluginObjectsStartedAt);
  const afterPluginNormalization = metricsRuntime
    ? captureProcessMetrics(metricsRuntime)
    : undefined;

  const parallelPoolInitializationStartedAt = metricsEnabled ? metricsTimestamp() : undefined;
  let parallelPluginInitResult: Awaited<ReturnType<typeof initializeParallelPlugins>> = undefined;
  let afterParallelPoolInitialization: ReturnType<typeof captureProcessMetrics> | undefined;
  try {
    parallelPluginInitResult = import.meta.browserBuild
      ? undefined
      : await initializeParallelPlugins(plugins, metricsRuntime, metricsId);
    if (
      !import.meta.browserBuild &&
      process.env.ROLLDOWN_PARALLEL_PLUGIN_METRICS_FAULT === 'create-after-pool-initialization'
    ) {
      throw new Error('injected metrics fault after parallel plugin pool initialization');
    }
    finishStage(stages, 'parallelPoolInitialization', parallelPoolInitializationStartedAt);
    afterParallelPoolInitialization = metricsRuntime
      ? captureProcessMetrics(metricsRuntime)
      : undefined;
  } catch (error) {
    await parallelPluginInitResult?.stopWorkers();
    throw error;
  }

  // Warn if deprecated experimental.strictExecutionOrder is used
  if ((inputOptions.experimental as any)?.strictExecutionOrder !== undefined) {
    console.warn(
      '`experimental.strictExecutionOrder` has been stabilized and moved to `output.strictExecutionOrder`. Please update your configuration.',
    );
  }

  try {
    const pluginContextConstructionStartedAt = metricsEnabled ? metricsTimestamp() : undefined;
    const pluginContextData = new PluginContextData(
      onLog,
      outputOptions,
      normalizedInputPlugins,
      normalizedOutputPlugins,
    );
    finishStage(stages, 'pluginContextConstruction', pluginContextConstructionStartedAt);

    // Convert `InputOptions` to `BindingInputOptions`
    const bindingifyInputOptionsStartedAt = metricsEnabled ? metricsTimestamp() : undefined;
    const bindingInputOptions = bindingifyInputOptions(
      plugins,
      inputOptions,
      outputOptions,
      pluginContextData,
      normalizedOutputPlugins,
      onLog,
      logLevel,
      watchMode,
      metricsEnabled ? pluginBindingMetrics : undefined,
    );
    finishStage(stages, 'bindingifyInputOptions', bindingifyInputOptionsStartedAt);
    const afterInputBindingification = metricsRuntime
      ? captureProcessMetrics(metricsRuntime)
      : undefined;

    // Convert `OutputOptions` to `BindingOutputOptions`
    const bindingifyOutputOptionsStartedAt = metricsEnabled ? metricsTimestamp() : undefined;
    const bindingOutputOptions = bindingifyOutputOptions(outputOptions, pluginContextData);
    finishStage(stages, 'bindingifyOutputOptions', bindingifyOutputOptionsStartedAt);
    const afterOutputBindingification = metricsRuntime
      ? captureProcessMetrics(metricsRuntime)
      : undefined;
    const atCreateBundlerOptionsFinish = metricsRuntime
      ? captureProcessMetrics(metricsRuntime)
      : undefined;
    const createBundlerOptionsFinishedAt = metricsEnabled ? metricsTimestamp() : undefined;

    if (
      metricsRuntime &&
      metricsId !== undefined &&
      createBundlerOptionsStartedAt &&
      createBundlerOptionsFinishedAt &&
      afterMetricsRuntimeSetupAtCreateBundlerOptionsStart &&
      afterPluginNormalization &&
      afterParallelPoolInitialization &&
      afterInputBindingification &&
      afterOutputBindingification &&
      atCreateBundlerOptionsFinish
    ) {
      const report: CreateBundlerOptionsMetrics = {
        kind: 'rolldown_create_bundler_options_metrics',
        version: 1,
        metricsId,
        measurementClass:
          'research-only instrumented initialization attribution; elapsed values are not uninstrumented wall evidence',
        pluginCounts: {
          inputBeforeOutputOptionsHook: inputPlugins.length,
          outputBeforeOutputOptionsHook: outputPlugins.length,
          ordinaryJs: pluginBindingMetrics.filter(({ pluginKind }) => pluginKind === 'ordinary-js')
            .length,
          parallelPlaceholders: pluginBindingMetrics.filter(
            ({ pluginKind }) => pluginKind === 'parallel-placeholder',
          ).length,
          builtin: pluginBindingMetrics.filter(({ pluginKind }) => pluginKind === 'builtin').length,
        },
        timeline: {
          createBundlerOptionsStartedAt,
          createBundlerOptionsFinishedAt,
        },
        stages,
        pluginBinding: pluginBindingMetrics,
        resources: {
          scope:
            'process CPU/RSS cover the whole process; heap and GC cover the main V8 isolate only',
          afterMetricsRuntimeSetupAtCreateBundlerOptionsStart,
          afterPluginNormalization,
          afterParallelPoolInitialization,
          afterInputBindingification,
          afterOutputBindingification,
          atCreateBundlerOptionsFinish,
        },
        isolationLimits: [
          'normalization stages may execute user plugin option promises and outputOptions hooks; they are deliberately reported separately but are not pure framework CPU',
          'metricsRuntimeSetup is research instrumentation overhead; the first main-isolate heap/GC snapshot can only be captured after that observer exists',
          'bindingifyInputOptions includes per-plugin bindingification plus non-plugin input option conversion; pluginBinding entries isolate elapsed per plugin but not native N-API materialization',
          'bindingifyOutputOptions is isolated as one stage because its nested callbacks and option conversions do not expose stable per-field boundaries',
          'whole-process RSS and CPU include existing native runtime threads and any already-created workers; only controlled differences support attribution',
        ],
      };
      writeValidatedMetrics(
        'rolldown-create-bundler-options-metrics',
        report,
        validateCreateBundlerOptionsMetrics,
      );
    }

    return {
      bundlerOptions: {
        inputOptions: bindingInputOptions,
        outputOptions: bindingOutputOptions,
        parallelPluginsRegistry: parallelPluginInitResult?.registry,
        ...(metricsId === undefined ? {} : { metricsId }),
      },
      inputOptions,
      onLog,
      stopWorkers: parallelPluginInitResult?.stopWorkers,
      finalizeParallelPluginMetricsAfterClose: parallelPluginInitResult?.finalizeMetricsAfterClose,
    };
  } catch (e) {
    await parallelPluginInitResult?.stopWorkers();
    throw e;
  }
}

function finishStage(
  stages: Record<string, MetricsStage>,
  name: string,
  startedAt: ReturnType<typeof metricsTimestamp> | undefined,
) {
  if (startedAt) stages[name] = metricsStage(startedAt, metricsTimestamp());
}

export interface BundlerOptionWithStopWorker {
  bundlerOptions: BindingBundlerOptions;
  inputOptions: InputOptions;
  onLog: LogHandler;
  stopWorkers?: () => Promise<void>;
  finalizeParallelPluginMetricsAfterClose?: () => Promise<void>;
}
