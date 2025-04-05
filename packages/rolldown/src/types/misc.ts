export interface RollupLog {
  binding?: string;
  cause?: unknown;
  code?: string;
  exporter?: string;
  frame?: string;
  hook?: string;
  id?: string;
  ids?: string[];
  loc?: {
    column: number;
    file?: string;
    line: number;
  };
  message: string;
  meta?: any;
  names?: string[];
  plugin?: string;
  pluginCode?: unknown;
  pos?: number;
  reexporter?: string;
  stack?: string;
  url?: string;
}

export interface RollupError extends RollupLog {
  name?: string;
  stack?: string;
  watchFiles?: string[];
}

export type LogLevel = 'warn' | 'info' | 'debug';
export type LogLevelOption = LogLevel | 'silent';

export type LoggingFunction = (
  log: RollupLog | string | (() => RollupLog | string),
) => void;

export type LoggingFunctionWithPosition = (
  log: RollupLog | string | (() => RollupLog | string),
  pos?: number | { column: number; line: number },
) => void;

export type LogHandler = (level: LogLevel, log: RollupLog) => void;

export type SourcemapPathTransformOption = (
  relativeSourcePath: string,
  sourcemapPath: string,
) => string;

export type SourcemapIgnoreListOption = (
  relativeSourcePath: string,
  sourcemapPath: string,
) => boolean;

export type WarningHandlerWithDefault = (
  warning: RollupLog,
  defaultHandler: LoggingFunction,
) => void;
