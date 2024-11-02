#[cfg(target_family = "wasm")]
mod wasm_shims {
  use oxc_index::{Idx, IndexVec};
  pub trait IndexVecExt<'data, I: Idx, T: 'data> {
    fn par_iter_mut_enumerated(&'data mut self) -> impl Iterator<Item = (I, &'data mut T)>;
  }

  impl<'data, I: Idx, T: 'data> IndexVecExt<'data, I, T> for IndexVec<I, T> {
    fn par_iter_mut_enumerated(&'data mut self) -> impl Iterator<Item = (I, &'data mut T)> {
      self.iter_mut_enumerated()
    }
  }
}

#[cfg(target_family = "wasm")]
pub use wasm_shims::*;

#[cfg(not(target_family = "wasm"))]
mod none_wasm {
  use oxc_index::{Idx, IndexVec};
  use rayon::iter::{IndexedParallelIterator, IntoParallelRefMutIterator, ParallelIterator};
  pub trait IndexVecExt<'data, I: Idx + Send, T: Send + 'data> {
    fn par_iter_mut_enumerated(&'data mut self) -> impl ParallelIterator<Item = (I, &'data mut T)>;
  }

  impl<'data, I: Idx + Send, T: Send + 'data> IndexVecExt<'data, I, T> for IndexVec<I, T> {
    fn par_iter_mut_enumerated(&'data mut self) -> impl ParallelIterator<Item = (I, &'data mut T)> {
      self.raw.par_iter_mut().enumerate().map(move |(i, t)| (I::from_usize(i), t))
    }
  }
}

#[cfg(not(target_family = "wasm"))]
pub use none_wasm::*;
