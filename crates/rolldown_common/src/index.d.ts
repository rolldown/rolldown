// Re-export types from other modules
export { Platform } from './inner_bundler_options/types/platform';
export {
  get_wasi_target_triple,
  is_wasi_platform,
  is_wasi_preview2,
} from './wasi_features';
