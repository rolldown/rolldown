/// The wall-clock window a hook's cost is estimated within.
///
/// Hook calls are timed on the Rust side with `Instant::elapsed()`. When calls run
/// concurrently they queue behind each other on the single JS thread, so each call's
/// measured duration includes the time it spent waiting for its predecessors. Summing
/// those durations overcounts by roughly half the queue depth: the thousands of
/// concurrent module tasks behind [`TimingSection::FetchModule`] inflate the sum by
/// orders of magnitude, while a serially-invoked callback is measured exactly. Raw
/// sums from those two regimes are not comparable, so a section that runs hooks
/// concurrently has its real elapsed span measured separately, and a hook's estimated
/// cost is its share of that section's measured hook time scaled by the section's
/// wall clock:
///
/// ```text
/// est(hook) = hook_micros / section_hook_micros * section_wall_micros
/// ```
///
/// The inflation factor is a property of the section's queue depth, so it is common
/// to every hook in the section and cancels in the ratio; multiplying by a real wall
/// clock re-grounds the result in seconds. Estimates are therefore additive,
/// comparable across sections, and bounded by the build.
///
/// The estimate apportions *all* of a section's wall clock to hooks, including the
/// Rust work inside it, so it over-attributes when a section is not plugin-bound.
/// That is why the report stays gated on `plugins_are_slow`, which establishes
/// plugin-boundedness independently of these numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimingSection {
  /// Hooks invoked one at a time, whose measured sum already *is* the elapsed time
  /// and is used as-is. Deliberately has no wall-clock boundary: any window wide
  /// enough to enclose a serial call site also contains unrelated Rust work, and
  /// apportioning that window would credit the callback for it.
  Serial,
  /// Per-module hooks, bounded by `ModuleLoader::fetch_modules`.
  FetchModule,
  /// Per-chunk addon hooks, bounded by `GenerateStage::instantiate_chunks`.
  InstantiateChunks,
  /// Per-chunk `renderChunk`, bounded by `render_chunks`.
  RenderChunks,
  /// Per-chunk `augmentChunkHash`, bounded by `augment_chunk_hash`.
  AugmentChunkHash,
  /// Hooks that fire outside the build window. Recorded, but excluded from the
  /// report: they are not part of the build time its percentages divide by.
  OutsideBuild,
}

impl TimingSection {
  /// Number of variants. Sections index a fixed-size array of measured wall clocks.
  pub(crate) const COUNT: usize = 6;
}

/// A hook whose execution time is attributed to its caller in `[PLUGIN_TIMINGS]`.
///
/// Each variant maps statically to the [`TimingSection`] it runs in, so call sites
/// pass only the hook and the section is derived — there is no way to pair a hook
/// with the wrong section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HookKind {
  ResolveId,
  ResolveDynamicImport,
  Load,
  Transform,
  TransformAst,
  ModuleParsed,
  BuildStart,
  BuildEnd,
  RenderStart,
  ResolveFileUrl,
  Banner,
  Footer,
  Intro,
  Outro,
  RenderChunk,
  AugmentChunkHash,
  RenderError,
  GenerateBundle,
  WriteBundle,
  CloseBundle,
  WatchChange,
  CloseWatcher,
  /// The `output.codeSplitting` / `advancedChunks` `groups[].name` chunk-name
  /// classifier. Not a plugin hook: the Rust core calls this user callback directly,
  /// so it is invisible to per-plugin timing yet can dominate a build.
  CodeSplittingName,
  /// The `groups[].test` predicate, when given as a function. Runs in the same loop as
  /// [`Self::CodeSplittingName`] and is invisible for the same reason.
  CodeSplittingTest,
}

impl HookKind {
  /// The section this hook runs in, and therefore how its measured time is turned
  /// into an estimate. See [`TimingSection`].
  pub const fn section(self) -> TimingSection {
    match self {
      Self::ResolveId
      | Self::ResolveDynamicImport
      | Self::Load
      | Self::Transform
      | Self::TransformAst
      | Self::ModuleParsed => TimingSection::FetchModule,
      Self::Banner | Self::Footer | Self::Intro | Self::Outro => TimingSection::InstantiateChunks,
      Self::RenderChunk => TimingSection::RenderChunks,
      Self::AugmentChunkHash => TimingSection::AugmentChunkHash,
      Self::WatchChange | Self::CloseWatcher => TimingSection::OutsideBuild,
      Self::BuildStart
      | Self::BuildEnd
      | Self::RenderStart
      // `resolve_file_urls` walks chunks and modules in nested `for` loops, awaiting
      // each call, so these never overlap.
      | Self::ResolveFileUrl
      | Self::RenderError
      | Self::GenerateBundle
      | Self::WriteBundle
      | Self::CloseBundle
      | Self::CodeSplittingName
      | Self::CodeSplittingTest => TimingSection::Serial,
    }
  }

  /// The name shown in the report, matching what a user writes in their config.
  pub const fn label(self) -> &'static str {
    match self {
      Self::ResolveId => "resolveId",
      Self::ResolveDynamicImport => "resolveDynamicImport",
      Self::Load => "load",
      Self::Transform => "transform",
      Self::TransformAst => "transformAst",
      Self::ModuleParsed => "moduleParsed",
      Self::BuildStart => "buildStart",
      Self::BuildEnd => "buildEnd",
      Self::RenderStart => "renderStart",
      Self::ResolveFileUrl => "resolveFileUrl",
      Self::Banner => "banner",
      Self::Footer => "footer",
      Self::Intro => "intro",
      Self::Outro => "outro",
      Self::RenderChunk => "renderChunk",
      Self::AugmentChunkHash => "augmentChunkHash",
      Self::RenderError => "renderError",
      Self::GenerateBundle => "generateBundle",
      Self::WriteBundle => "writeBundle",
      Self::CloseBundle => "closeBundle",
      Self::WatchChange => "watchChange",
      Self::CloseWatcher => "closeWatcher",
      Self::CodeSplittingName => "codeSplitting groups[].name",
      Self::CodeSplittingTest => "codeSplitting groups[].test",
    }
  }
}
