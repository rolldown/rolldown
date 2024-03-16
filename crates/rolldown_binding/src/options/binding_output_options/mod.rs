use napi_derive::napi;
use serde::Deserialize;

#[napi(object)]
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BindingOutputOptions {
  // --- Options Rolldown doesn't need to be supported
  // /** @deprecated Use the "renderDynamicImport" plugin hook instead. */
  // dynamicImportFunction: string | undefined;
  pub entry_file_names: Option<String>,
  pub chunk_file_names: Option<String>,

  // amd: NormalizedAmdOptions;
  // assetFileNames: string | ((chunkInfo: PreRenderedAsset) => string);
  // banner: () => string | Promise<string>;
  // chunkFileNames: string | ((chunkInfo: PreRenderedChunk) => string);
  // compact: boolean;
  pub dir: Option<String>,
  // pub entry_file_names: String, // | ((chunkInfo: PreRenderedChunk) => string)
  // esModule: boolean;
  #[napi(ts_type = "'default' | 'named' | 'none' | 'auto'")]
  pub exports: Option<String>,
  // extend: boolean;
  // externalLiveBindings: boolean;
  // footer: () => string | Promise<string>;
  #[napi(ts_type = "'esm' | 'cjs'")]
  pub format: Option<String>,
  // freeze: boolean;
  // generatedCode: NormalizedGeneratedCodeOptions;
  // globals: GlobalsOption;
  // hoistTransitiveImports: boolean;
  // indent: true | string;
  // inlineDynamicImports: boolean;
  // interop: GetInterop;
  // intro: () => string | Promise<string>;
  // manualChunks: ManualChunksOption;
  // minifyInternalExports: boolean;
  // name: string | undefined;
  // namespaceToStringTag: boolean;
  // noConflict: boolean;
  // outro: () => string | Promise<string>;
  // paths: OptionsPaths;
  // plugins: OutputPlugin[];
  // preferConst: boolean;
  // preserveModules: boolean;
  // preserveModulesRoot: string | undefined;
  // sanitizeFileName: (fileName: string) => string;
  #[napi(ts_type = "'file' | 'inline' | 'hidden'")]
  pub sourcemap: Option<String>,
  // sourcemapExcludeSources: boolean;
  // sourcemapFile: string | undefined;
  // sourcemapPathTransform: SourcemapPathTransformOption | undefined;
  // strict: boolean;
  // systemNullSetters: boolean;
  // validate: boolean;
  // --- Enhanced options
  // pub minify: bool,
}

impl From<BindingOutputOptions> for rolldown::OutputOptions {
  fn from(value: BindingOutputOptions) -> Self {
    let mut options = Self::default();

    if let Some(entry_file_names) = value.entry_file_names {
      options.entry_file_names = rolldown::FileNameTemplate::from(entry_file_names);
    }

    if let Some(chunk_file_names) = value.chunk_file_names {
      options.chunk_file_names = rolldown::FileNameTemplate::from(chunk_file_names);
    }

    if let Some(dir) = value.dir {
      options.dir = dir;
    }

    if let Some(sourcemap) = value.sourcemap {
      options.sourcemap = sourcemap.into();
    }

    options
  }
}
