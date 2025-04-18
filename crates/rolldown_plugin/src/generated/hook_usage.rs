// Auto-generated code, DO NOT EDIT DIRECTLY!
// To edit this generated file you have to edit `tasks/generator/src/generators/hook_usage.rs`

use bitflags::bitflags;
bitflags! {
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
  pub struct HookUsage: u32 {
    const BuildStart = 1 << 0;
    const ResolveId = 1 << 1;
    const ResolveDynamicImport = 1 << 2;
    const Load = 1 << 3;
    const Transform = 1 << 4;
    const ModuleParsed = 1 << 5;
    const BuildEnd = 1 << 6;
    const RenderStart = 1 << 7;
    const RenderError = 1 << 8;
    const RenderChunk = 1 << 9;
    const AugmentChunkHash = 1 << 10;
    const GenerateBundle = 1 << 11;
    const WriteBundle = 1 << 12;
    const CloseBundle = 1 << 13;
    const WatchChange = 1 << 14;
    const CloseWatcher = 1 << 15;
    const TransformAst = 1 << 16;
    const Banner = 1 << 17;
    const Footer = 1 << 18;
    const Intro = 1 << 19;
    const Outro = 1 << 20;
  }
}
