#[derive(ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct ChunkGraphReady {
  #[ts(type = "'ChunkGraphReady'")]
  pub action: &'static str,
  pub chunks: Vec<Chunk>,
}

#[derive(ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct Chunk {
  pub id: u32,
  /// ```js
  /// import { defineConfig } from 'rolldown';
  /// export default defineConfig({
  ///   input: {
  ///     main: './index.ts',
  ///   },
  ///   output: {
  ///     advancedChunks: {
  ///       groups: [
  ///         {
  ///           name: 'npm-libs',
  ///           test: /node_modules/,
  ///         },
  ///       ],
  ///     },
  ///   },
  /// });
  /// ```
  /// - `main` is the name, if this chunk is an entry chunk.
  /// - `npm-libs` is the name, if this chunk is created by `output.advancedChunks`.
  pub name: Option<String>,

  /// ```js
  /// import { defineConfig } from 'rolldown';
  /// export default defineConfig({
  ///   input: {
  ///     main: './index.ts',
  ///   },
  ///   output: {
  ///     advancedChunks: {
  ///       groups: [
  ///         {
  ///           name: 'npm-libs',
  ///           test: /node_modules/,
  ///         },
  ///       ],
  ///     },
  ///   },
  /// });
  /// ```
  /// - `group_index` will be `0` if this chunk is created by `output.advancedChunks`.
  pub group_index: Option<u32>,
  pub is_user_defined_entry: bool,
  /// A entry could be both user-defined and async.
  pub is_async_entry: bool,
  pub entry_module: Option<String>,
  pub modules: Vec<String>,
  #[ts(type = "'advanced-chunks' | 'preserve-modules' | 'entry' | 'common'")]
  pub reason: &'static str,
  pub imports: Vec<ChunkImport>,
}

#[derive(ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct ChunkImport {
  /// Id of the imported chunk
  pub id: u32,
  #[ts(type = "'import-statement' | 'dynamic-import'")]
  pub kind: &'static str,
}
