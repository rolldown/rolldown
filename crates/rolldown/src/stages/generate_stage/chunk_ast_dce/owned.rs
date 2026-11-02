use std::{marker::PhantomPinned, pin::Pin, sync::Arc};

use oxc::{allocator::Allocator, ast::ast::Program};
use rolldown_common::{ModuleRenderOutput, ModuleTable, NormalModule, NormalizedBundlerOptions};
use rolldown_ecmascript::EcmaAst;

use super::{ChunkAstDceOutput, dce_chunk, global};
use crate::types::generator::GenerateContext;

/// Private lifetime-erased root. Before assembly, roots have disjoint donor arenas, so rebasing
/// may transfer exclusive roots to Rayon workers. After assembly, only the frozen owner prints
/// them; no parallel mutation or allocator access is allowed.
pub(super) struct ChunkProgram(pub(super) Program<'static>);

// SAFETY: only pre-assembly rebasing transfers individual roots. The enclosing owner keeps all
// backing storage at stable addresses and joins those workers before mixing arenas.
unsafe impl Send for ChunkProgram {}

impl ChunkProgram {
  pub(super) fn take_body<'a>(
    &mut self,
    allocator: &'a Allocator,
  ) -> oxc::allocator::Vec<'a, oxc::ast::ast::Statement<'a>> {
    // SAFETY: the scratch allocator belongs to the same ChunkAst and outlives this root.
    let empty: oxc::allocator::Vec<'static, oxc::ast::ast::Statement<'static>> = unsafe {
      std::mem::transmute(oxc::allocator::Vec::<oxc::ast::ast::Statement<'_>>::new_in(&allocator))
    };
    std::mem::replace(&mut self.0.body, empty)
  }
}

impl std::ops::Deref for ChunkProgram {
  type Target = Program<'static>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl std::ops::DerefMut for ChunkProgram {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

/// An exclusive chunk under construction. Fields are private: programs can borrow any donor
/// arena, the scratch arena, and `source_text`, and must never leave this owner.
/// Field order also keeps all backing storage alive while programs are dropped on unwind.
/// See internal-docs/chunk-ast-minify/implementation.md.
pub(in crate::stages::generate_stage) struct ChunkAst {
  programs: Vec<ChunkProgram>,
  assembled: Option<ChunkProgram>,
  donors: Vec<EcmaAst>,
  allocator: Box<Allocator>,
  source_text: Option<Arc<str>>,
  /// Finalized per-carrier donors follow the file's own modules, grouped in registration order.
  inline_records: Vec<(rolldown_common::ChunkIdx, std::ops::Range<usize>)>,
  _pin: PhantomPinned,
}

// SAFETY: Moving the pinned owner transfers all programs and their backing storage together. No
// borrowed program/allocator escapes; the source and arena allocations have stable addresses.
unsafe impl Send for ChunkAst {}

impl ChunkAst {
  pub(in crate::stages::generate_stage) fn new(capacity: usize) -> Pin<Box<Self>> {
    Box::pin(Self {
      programs: Vec::with_capacity(capacity),
      assembled: None,
      donors: Vec::with_capacity(capacity),
      allocator: Box::default(),
      source_text: None,
      inline_records: Vec::new(),
      _pin: PhantomPinned,
    })
  }

  pub(in crate::stages::generate_stage) fn push(self: &mut Pin<Box<Self>>, ast: EcmaAst) {
    // SAFETY: no field is moved. The exact capacity reserved by `new` keeps both Vec buffers
    // stable, so existing roots cannot be relocated by this push.
    let this = unsafe { self.as_mut().get_unchecked_mut() };
    assert!(this.programs.len() < this.programs.capacity(), "chunk root capacity is exact");
    assert!(this.donors.len() < this.donors.capacity(), "chunk donor capacity is exact");
    // Place the donor at its final address before detaching a root that borrows its allocator.
    this.donors.push(ast);
    let ast = this.donors.last_mut().expect("just pushed donor");
    let program = ast.program.with_mut(|fields| {
      let empty = Program::new(
        oxc::span::SPAN,
        fields.program.source_type,
        "",
        [],
        None,
        [],
        [],
        &oxc::ast::builder::AstBuilder::new(fields.allocator),
      );
      let program = std::mem::replace(fields.program, empty);
      // SAFETY: the original ProgramCell stays intact in `donors`, keeping its allocator stable.
      // Only this private pinned owner accesses the detached program, and every backing owner
      // outlives all programs.
      unsafe { std::mem::transmute::<Program<'_>, Program<'static>>(program) }
    });
    this.programs.push(ChunkProgram(program));
  }

  pub(in crate::stages::generate_stage) fn push_inline(
    self: &mut Pin<Box<Self>>,
    carried: &mut [crate::stages::generate_stage::CarriedRender],
  ) {
    for record in carried {
      let start = self.programs.len();
      for module in &mut record.modules {
        self.push(module.ast.take().expect("finalized inline copy consumed once"));
      }
      let end = self.programs.len();
      // SAFETY: only the independent inline-record Vec is mutated; no owner field is moved.
      unsafe { self.as_mut().get_unchecked_mut() }.inline_records.push((record.record, start..end));
    }
  }

  pub(in crate::stages::generate_stage) fn prepare(
    mut self: Pin<Box<Self>>,
    ctx: &GenerateContext<'_>,
    options: &oxc::minifier::MinifierOptions,
  ) -> PreparedChunk {
    if matches!(
      ctx.options.format,
      rolldown_common::OutputFormat::Iife | rolldown_common::OutputFormat::Umd
    ) || ctx.options.is_dev_mode_enabled()
      || ctx.inline_state.has_records()
      || super::defer_for_render_hooks(ctx)
    {
      return PreparedChunk { ast: self, output: ChunkAstDceOutput::default(), deferred: true };
    }
    let output = {
      // SAFETY: assembly borrows fields in place; no owner or Vec storage is moved.
      let this = unsafe { self.as_mut().get_unchecked_mut() };
      dce_chunk(
        ctx,
        &this.allocator,
        &mut this.programs,
        &mut this.assembled,
        &mut this.source_text,
        options,
      )
    };
    PreparedChunk { ast: self, output, deferred: false }
  }
}

/// Frozen after compression. Sharing only permits codegen; it exposes neither mutable nodes
/// (including their interior semantic cells) nor allocators to callers.
pub(in crate::stages::generate_stage) struct PreparedChunk {
  ast: Pin<Box<ChunkAst>>,
  output: ChunkAstDceOutput,
  deferred: bool,
}

// SAFETY: every program and all arenas move together in `ChunkAst`.
unsafe impl Send for PreparedChunk {}
// SAFETY: preparation has finished before publication. The sole shared operation is read-only
// codegen, which does not write AST cells or allocate into any donor/scratch arena. Rayon joins
// all readers before this owner can be consumed/dropped. No AST or allocator references escape.
unsafe impl Sync for PreparedChunk {}

impl PreparedChunk {
  pub(in crate::stages::generate_stage) fn print_module(
    &self,
    pos: usize,
    module: &NormalModule,
    options: &NormalizedBundlerOptions,
    initial_indent: u32,
    module_table: &ModuleTable,
  ) -> ModuleRenderOutput {
    #[cfg(feature = "testing")]
    super::record_coverage(
      options,
      "print_module",
      if self.output.space.is_some() { "global" } else { "preliminary" },
      Some(module.id.as_str()),
    );
    match self.output.space.as_deref() {
      Some(space) => global::print_module(
        module,
        &self.ast.programs[pos].0,
        options,
        initial_indent,
        space,
        pos,
        module_table,
      ),
      None => {
        let comments = options.comments;
        let ret = oxc::codegen::Codegen::new()
          .with_options(oxc::codegen::CodegenOptions {
            initial_indent,
            comments: oxc::codegen::CommentOptions {
              normal: false,
              legal: if comments.legal {
                oxc::codegen::LegalComment::Inline
              } else {
                oxc::codegen::LegalComment::None
              },
              jsdoc: comments.jsdoc,
              annotation: self.deferred || comments.annotation,
            },
            ascii_only: options.charset.is_ascii(),
            source_map_path: (options.sourcemap.is_some() && !module.is_virtual())
              .then(|| std::path::PathBuf::from(module.id.as_str())),
            ..Default::default()
          })
          .build(&self.ast.programs[pos].0);
        ModuleRenderOutput {
          code: ret.code,
          map: ret.map.map(oxc_sourcemap::SourceMap::into_owned),
          map_is_final: false,
        }
      }
    }
  }

  pub(in crate::stages::generate_stage) fn into_render_state(
    self,
    ctx: &GenerateContext<'_>,
  ) -> (Option<ChunkAstDceOutput>, Option<PendingChunk>) {
    if !self.deferred {
      return (Some(self.output), None);
    }
    assert!(!matches!(
      ctx.options.format,
      rolldown_common::OutputFormat::Iife | rolldown_common::OutputFormat::Umd
    ));
    let mut ast = self.ast;
    let (program, layout) = {
      // SAFETY: all assembled references remain inside the pinned owner.
      let this = unsafe { ast.as_mut().get_unchecked_mut() };
      let generated = super::generated_text(ctx, &this.allocator);
      let modules: Vec<_> = ctx
        .chunk
        .modules
        .iter()
        .filter_map(|idx| ctx.link_output.module_table[*idx].as_normal())
        .collect();
      global::assemble_chunk(
        ctx,
        &this.allocator,
        &mut this.programs,
        &modules,
        &generated,
        &mut this.source_text,
      )
      .expect("flat generated glue has no parsed wrapper")
    };
    // SAFETY: the complete root stays private to the same pinned owner as its donor roots.
    unsafe { ast.as_mut().get_unchecked_mut() }.assembled =
      Some(ChunkProgram(unsafe { std::mem::transmute::<Program<'_>, Program<'static>>(program) }));
    (
      None,
      Some(PendingChunk {
        ast,
        layout,
        prefix: String::new(),
        suffix: String::new(),
        wrapped: false,
      }),
    )
  }

  pub(in crate::stages::generate_stage) fn into_wrapped(
    self,
    ctx: &GenerateContext<'_>,
    plan: crate::ecmascript::WrappedPlan,
  ) -> rolldown_error::BuildResult<PendingChunk> {
    let mut ast = self.ast;
    let wrapper = plan.ast.expect("builtin renderer supplies a structured plan");
    let (program, layout) = {
      // SAFETY: all assembled references remain inside the pinned owner.
      let this = unsafe { ast.as_mut().get_unchecked_mut() };
      let generated = super::GeneratedText {
        prefix: Some(this.allocator.alloc_str(&wrapper.outer_prefix)),
        imports: Some(this.allocator.alloc_str(&plan.imports)),
        exports: Some(this.allocator.alloc_str(&wrapper.tail)),
        imports_after_runtime: plan.runtime_before_imports,
      };
      let modules: Vec<_> = ctx
        .chunk
        .modules
        .iter()
        .copied()
        .chain(
          this
            .inline_records
            .iter()
            .flat_map(|(record, _)| ctx.chunk_graph.chunk_table[*record].modules.iter().copied()),
        )
        .filter_map(|idx| ctx.link_output.module_table[idx].as_normal())
        .collect();
      global::assemble_chunk_with_wrapper(
        ctx,
        &this.allocator,
        &mut this.programs,
        &modules,
        &generated,
        &mut this.source_text,
        &this.inline_records,
        Some(&wrapper),
      )?
    };
    // SAFETY: the root remains inside the same pinned owner as every donor and the scratch arena.
    unsafe { ast.as_mut().get_unchecked_mut() }.assembled =
      Some(ChunkProgram(unsafe { std::mem::transmute::<Program<'_>, Program<'static>>(program) }));
    Ok(PendingChunk { ast, layout, prefix: String::new(), suffix: String::new(), wrapped: true })
  }
}

pub struct PendingChunk {
  ast: Pin<Box<ChunkAst>>,
  layout: global::ChunkLayout,
  prefix: String,
  suffix: String,
  wrapped: bool,
}

type ModuleSources = Option<Arc<[Box<dyn rolldown_sourcemap::Source + Send + Sync>]>>;

pub(in crate::stages::generate_stage) struct OptimizedChunk {
  pub code: String,
  pub map: Option<rolldown_sourcemap::SourceMap>,
  pub property_cache: Option<oxc::minifier::ManglePropertyCache>,
  pub modules: Vec<(rolldown_common::ModuleIdx, ModuleSources)>,
}

impl PendingChunk {
  pub(crate) fn set_envelope(&mut self, prefix: String, suffix: String) {
    self.prefix = prefix;
    self.suffix = suffix;
  }

  pub(in crate::stages::generate_stage) fn optimize(
    mut self,
    options: &NormalizedBundlerOptions,
    module_table: &ModuleTable,
    filename: &str,
    minify_options: &oxc::minifier::MinifierOptions,
  ) -> OptimizedChunk {
    #[cfg(not(feature = "testing"))]
    let _ = filename;
    let ctx = &global::OptimizeContext {
      options,
      module_table,
      #[cfg(feature = "testing")]
      filename: Some(filename),
    };
    // SAFETY: optimization mutates fields in place without moving the pinned owner.
    let ast = unsafe { self.ast.as_mut().get_unchecked_mut() };
    let program = &mut ast.assembled.as_mut().expect("assembled pending program").0;
    // SAFETY: shorten the lifetime-erased root to the stable scratch allocator borrow.
    let program =
      unsafe { std::mem::transmute::<&mut Program<'static>, &mut Program<'_>>(program) };
    if self.wrapped {
      return global::optimize_wrapped(ctx, &ast.allocator, program, self.layout, minify_options);
    }
    let output = global::optimize_chunk(
      ctx,
      &ast.allocator,
      &mut ast.programs,
      program,
      self.layout,
      minify_options,
    );
    let space = output.space.as_ref().expect("global chunk source space");
    let module_sources: Vec<_> = ast
      .programs
      .iter()
      .enumerate()
      .map(|(pos, root)| {
        let module = ctx.module_table[space.module_idx(pos)].as_normal().expect("normal donor");
        let rendered =
          global::print_module(module, &root.0, ctx.options, 0, space, pos, ctx.module_table);
        // Preliminary rendering already reported transform-chain warnings before hooks.
        (
          module.idx,
          crate::utils::render_ecma_module::render_ecma_module(module, ctx.options, rendered)
            .sources,
        )
      })
      .collect();
    let mut joiner = rolldown_sourcemap::SourceJoiner::default();
    if !self.prefix.is_empty() {
      joiner.append_source(self.prefix);
    }
    if let Some(code) = output.imports {
      if let Some(map) = output.imports_map {
        joiner.append_source(rolldown_sourcemap::SourceMapSource::new(code, map));
      } else {
        joiner.append_source(code);
      }
    }
    for (_, sources) in &module_sources {
      if let Some(sources) = sources {
        for source in sources.iter() {
          joiner.append_source(source);
        }
      }
    }
    if let Some(code) = output.exports {
      if let Some(map) = output.exports_map {
        joiner.append_source(rolldown_sourcemap::SourceMapSource::new(code, map));
      } else {
        joiner.append_source(code);
      }
    }
    if !self.suffix.is_empty() {
      joiner.append_source(self.suffix);
    }
    let (code, map) = joiner.join();
    drop(joiner);
    OptimizedChunk {
      code,
      map: map.map(global::dedupe_sources),
      property_cache: output.property_mangle_cache,
      modules: module_sources,
    }
  }
}

#[cfg(test)]
mod tests {
  use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    pin::Pin,
    sync::{Arc, Weak},
  };

  use oxc::{allocator::Vec as ArenaVec, codegen::Codegen, span::SourceType};
  use rolldown_ecmascript::EcmaCompiler;

  use super::{ChunkAst, ChunkAstDceOutput, PreparedChunk};

  fn mixed_chunk() -> Pin<Box<ChunkAst>> {
    let mut chunk = ChunkAst::new(2);
    for source in ["console.log([1, 2]);", "console.log({ x: 3 });"] {
      chunk.push(EcmaCompiler::parse("test.js", source.to_owned(), SourceType::mjs()).unwrap());
    }
    // SAFETY: the test exercises the same pinned ownership boundary as preparation.
    let this = unsafe { chunk.as_mut().get_unchecked_mut() };
    // Build a scratch-backed vector containing roots from BOTH donor arenas.
    let mut body = ArenaVec::new_in(&this.allocator.as_ref());
    for root in &mut this.programs {
      body.extend(root.take_body(&this.allocator));
    }
    this.programs[0].body = unsafe {
      std::mem::transmute::<
        ArenaVec<'_, oxc::ast::ast::Statement<'_>>,
        ArenaVec<'static, oxc::ast::ast::Statement<'static>>,
      >(body)
    };
    this.source_text = Some(Arc::from("console.log([1, 2]);\nconsole.log({ x: 3 });"));
    let text = this.source_text.as_deref().unwrap();
    // SAFETY: retained by chunk before any root references it.
    let text = unsafe { std::mem::transmute::<&str, &'static str>(text) };
    for root in &mut this.programs {
      root.source_text = text;
    }
    chunk
  }

  #[test]
  fn backing_allocators_stay_stable_across_owner_moves() {
    let mut chunk = mixed_chunk();
    let scratch = std::ptr::from_ref(chunk.allocator.as_ref());
    let donor = std::ptr::from_ref(&chunk.donors[0].program.borrow_owner().allocator);
    // Heap transfer and thread transfer must not relocate allocators embedded in arena vectors.
    let moved = chunk;
    chunk = std::thread::spawn(move || moved).join().unwrap_or_else(|_| panic!("owner transfer"));
    assert_eq!(scratch, std::ptr::from_ref(chunk.allocator.as_ref()));
    assert_eq!(donor, std::ptr::from_ref(&chunk.donors[0].program.borrow_owner().allocator));
    // SAFETY: the test mutates roots in place without moving their pinned owner.
    let this = unsafe { chunk.as_mut().get_unchecked_mut() };
    // Nested vectors still refer to donor arenas; the outer vector refers to scratch.
    let body = this.programs[0].take_body(&this.allocator);
    let mut body = body;
    body.reserve(1024);
    assert_eq!(body.len(), 2);
    assert_eq!(Codegen::new().build(&this.programs[1].0).code, "");
  }

  #[test]
  fn mixed_roots_remain_readable_during_unwind() {
    struct ReadOnUnwind<'a>(&'a ChunkAst, Weak<str>);
    impl Drop for ReadOnUnwind<'_> {
      fn drop(&mut self) {
        assert!(self.1.upgrade().is_some());
        let code = Codegen::new().build(&self.0.programs[0].0).code;
        assert!(code.contains("console.log([1, 2])"));
        assert!(code.contains("x: 3"));
      }
    }
    let mut weak: Option<Weak<str>> = None;
    let result = catch_unwind(AssertUnwindSafe(|| {
      let chunk = mixed_chunk();
      weak = Some(Arc::downgrade(chunk.source_text.as_ref().unwrap()));
      let _read = ReadOnUnwind(chunk.as_ref().get_ref(), Weak::clone(weak.as_ref().unwrap()));
      panic!("preparation failed after cross-arena assembly");
    }));
    assert!(result.is_err());
    assert!(weak.unwrap().upgrade().is_none(), "unwind must release the shared source");
  }

  #[test]
  fn frozen_owner_supports_joined_parallel_readers() {
    let chunk =
      PreparedChunk { ast: mixed_chunk(), output: ChunkAstDceOutput::default(), deferred: false };
    std::thread::scope(|scope| {
      for _ in 0..8 {
        let chunk = &chunk;
        scope.spawn(move || {
          let code = Codegen::new().build(&chunk.ast.programs[0].0).code;
          assert!(code.contains("console.log"));
        });
      }
    });
  }
}
