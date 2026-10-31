use std::sync::Arc;

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
  donors: Vec<EcmaAst>,
  allocator: Box<Allocator>,
  source_text: Option<Arc<str>>,
}

// SAFETY: Moving the owner transfers all programs and their backing storage together. No
// borrowed program/allocator escapes; the source and arena allocations have stable addresses.
unsafe impl Send for ChunkAst {}

impl ChunkAst {
  pub(in crate::stages::generate_stage) fn new(capacity: usize) -> Self {
    Self {
      programs: Vec::with_capacity(capacity),
      donors: Vec::with_capacity(capacity),
      allocator: Box::default(),
      source_text: None,
    }
  }

  pub(in crate::stages::generate_stage) fn push(&mut self, mut ast: EcmaAst) {
    // Reserve before detaching a program so allocation failure cannot leave its storage behind.
    self.programs.reserve(1);
    self.donors.reserve(1);
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
      // SAFETY: the original ProgramCell stays intact in `donors` below, keeping its allocator
      // stable. Only this private owner accesses the detached program, and every backing
      // owner outlives all programs.
      unsafe { std::mem::transmute::<Program<'_>, Program<'static>>(program) }
    });
    self.donors.push(ast);
    self.programs.push(ChunkProgram(program));
  }

  pub(in crate::stages::generate_stage) fn prepare(
    mut self,
    ctx: &GenerateContext<'_>,
    options: &oxc::minifier::MinifierOptions,
  ) -> PreparedChunk {
    let output =
      dce_chunk(ctx, &self.allocator, &mut self.programs, &mut self.source_text, options);
    PreparedChunk { ast: self, output }
  }
}

/// Frozen after compression. Sharing only permits codegen; it exposes neither mutable nodes
/// (including their interior semantic cells) nor allocators to callers.
pub(in crate::stages::generate_stage) struct PreparedChunk {
  ast: ChunkAst,
  output: ChunkAstDceOutput,
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
      if self.output.space.is_some() { "global" } else { "legacy" },
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
              annotation: comments.annotation,
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

  pub(in crate::stages::generate_stage) fn into_output(self) -> ChunkAstDceOutput {
    self.output
  }
}

#[cfg(test)]
mod tests {
  use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Weak},
  };

  use oxc::{allocator::Vec as ArenaVec, codegen::Codegen, span::SourceType};
  use rolldown_ecmascript::EcmaCompiler;

  use super::{ChunkAst, ChunkAstDceOutput, PreparedChunk};

  fn mixed_chunk() -> ChunkAst {
    let mut chunk = ChunkAst::new(2);
    for source in ["console.log([1, 2]);", "console.log({ x: 3 });"] {
      chunk.push(EcmaCompiler::parse("test.js", source.to_owned(), SourceType::mjs()).unwrap());
    }
    // Build a scratch-backed vector containing roots from BOTH donor arenas.
    let mut body = ArenaVec::new_in(&chunk.allocator.as_ref());
    for root in &mut chunk.programs {
      body.extend(root.take_body(&chunk.allocator));
    }
    // SAFETY: the test exercises the same private ownership boundary as preparation.
    chunk.programs[0].body = unsafe {
      std::mem::transmute::<
        ArenaVec<'_, oxc::ast::ast::Statement<'_>>,
        ArenaVec<'static, oxc::ast::ast::Statement<'static>>,
      >(body)
    };
    chunk.source_text = Some(Arc::from("console.log([1, 2]);\nconsole.log({ x: 3 });"));
    let text = chunk.source_text.as_deref().unwrap();
    // SAFETY: retained by chunk before any root references it.
    let text = unsafe { std::mem::transmute::<&str, &'static str>(text) };
    for root in &mut chunk.programs {
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
    let moved = Box::new(chunk);
    chunk = std::thread::spawn(move || *moved).join().unwrap_or_else(|_| panic!("owner transfer"));
    assert_eq!(scratch, std::ptr::from_ref(chunk.allocator.as_ref()));
    assert_eq!(donor, std::ptr::from_ref(&chunk.donors[0].program.borrow_owner().allocator));
    // Nested vectors still refer to donor arenas; the outer vector refers to scratch.
    let body = chunk.programs[0].take_body(&chunk.allocator);
    let mut body = body;
    body.reserve(1024);
    assert_eq!(body.len(), 2);
    assert_eq!(Codegen::new().build(&chunk.programs[1].0).code, "");
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
      let _read = ReadOnUnwind(&chunk, Weak::clone(weak.as_ref().unwrap()));
      panic!("preparation failed after cross-arena assembly");
    }));
    assert!(result.is_err());
    assert!(weak.unwrap().upgrade().is_none(), "unwind must release the shared source");
  }

  #[test]
  fn frozen_owner_supports_joined_parallel_readers() {
    let chunk = PreparedChunk { ast: mixed_chunk(), output: ChunkAstDceOutput::default() };
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
