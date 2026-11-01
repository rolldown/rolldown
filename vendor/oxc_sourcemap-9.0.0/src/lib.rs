mod compose;
mod concat_sourcemap_builder;
mod decode;
mod encode;
mod error;
mod owned_sourcemap;
mod sourcemap;
mod sourcemap_builder;
mod sourcemap_visualizer;
mod token;

#[cfg(feature = "napi")]
pub mod napi;

pub use concat_sourcemap_builder::ConcatSourceMapBuilder;
pub use decode::JSONSourceMap;
pub use error::Error;
pub use owned_sourcemap::OwnedSourceMap;
pub use sourcemap::{SourceMap, SourceMapParts};
pub use sourcemap_builder::SourceMapBuilder;
pub use sourcemap_visualizer::SourcemapVisualizer;
pub use token::{SourceViewToken, Token, TokenChunk};
