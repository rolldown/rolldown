import type { BindingViteHtmlPluginConfig } from '../binding.cjs';
import type { MinimalPluginContext } from '../plugin/minimal-plugin-context';
import type { OutputBundle } from '../types/output-bundle';
import type { OutputChunk } from '../types/rolldown-output';
import { BuiltinPlugin } from './utils';

interface HtmlTagDescriptor {
  tag: string;
  /**
   * attribute values will be escaped automatically if needed
   */
  attrs?: Record<string, string | boolean | undefined>;
  children?: string | HtmlTagDescriptor[];
  /**
   * default: 'head-prepend'
   */
  injectTo?: 'head' | 'body' | 'head-prepend' | 'body-prepend';
}

type IndexHtmlTransformResult =
  | string
  | HtmlTagDescriptor[]
  | {
    html: string;
    tags: HtmlTagDescriptor[];
  };

type IndexHtmlTransformHook = (
  this: MinimalPluginContext,
  html: string,
  ctx: IndexHtmlTransformContext,
) => IndexHtmlTransformResult | void | Promise<IndexHtmlTransformResult | void>;

export interface IndexHtmlTransformContext {
  /**
   * public path when served
   */
  path: string;
  /**
   * filename on disk
   */
  filename: string;
  bundle?: OutputBundle;
  chunk?: OutputChunk;
}

export interface ViteHtmlPluginOptions extends BindingViteHtmlPluginConfig {
  preHooks: IndexHtmlTransformHook[];
  normalHooks: IndexHtmlTransformHook[];
  postHooks: IndexHtmlTransformHook[];
  applyHtmlTransforms: (
    html: string,
    hooks: IndexHtmlTransformHook[],
    pluginContext: MinimalPluginContext,
    ctx: IndexHtmlTransformContext,
  ) => Promise<string>;
}

export function viteHtmlPlugin(
  config?: ViteHtmlPluginOptions,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:vite-html', config);
}
