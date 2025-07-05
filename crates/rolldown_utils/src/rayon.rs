#[cfg(target_family = "wasm")]
mod wasm_shims {
  use std::iter::Flatten;
  pub use std::iter::Iterator as ParallelIterator;

  pub trait ParallelBridge: Sized {
    fn par_bridge(self) -> Self;
  }

  pub trait IteratorExt: Sized {
    fn flatten_iter(self) -> Flatten<Self>
    where
      Self: Iterator<Item: IntoIterator>;
  }

  impl<T: Iterator + Send> IteratorExt for T {
    fn flatten_iter(self) -> Flatten<Self>
    where
      Self: Iterator<Item: IntoIterator>,
    {
      self.flatten()
    }
  }

  impl<T: Iterator + Send> ParallelBridge for T {
    fn par_bridge(self) -> Self {
      self
    }
  }

  pub trait IntoParallelIterator: Sized {
    type Item;
    type Iter: Iterator<Item = Self::Item>;

    fn into_par_iter(self) -> Self::Iter;
  }

  impl<I> IntoParallelIterator for I
  where
    I: IntoIterator,
  {
    type Item = I::Item;
    type Iter = I::IntoIter;

    fn into_par_iter(self) -> Self::Iter {
      self.into_iter()
    }
  }

  pub trait IntoParallelRefIterator<'data> {
    type Item: 'data;
    type Iter: ParallelIterator<Item = Self::Item>;

    fn par_iter(&'data self) -> Self::Iter;
  }

  impl<'data, I: 'data + ?Sized> IntoParallelRefIterator<'data> for I
  where
    &'data I: IntoParallelIterator,
  {
    type Iter = <&'data I as IntoParallelIterator>::Iter;
    type Item = <&'data I as IntoParallelIterator>::Item;

    fn par_iter(&'data self) -> Self::Iter {
      self.into_par_iter()
    }
  }

  pub trait IntoParallelRefMutIterator<'data> {
    type Item: 'data;
    type Iter: ParallelIterator<Item = Self::Item>;

    fn par_iter_mut(&'data mut self) -> Self::Iter;
  }

  impl<'data, I: 'data + ?Sized> IntoParallelRefMutIterator<'data> for I
  where
    &'data mut I: IntoParallelIterator,
  {
    type Iter = <&'data mut I as IntoParallelIterator>::Iter;
    type Item = <&'data mut I as IntoParallelIterator>::Item;

    fn par_iter_mut(&'data mut self) -> Self::Iter {
      self.into_par_iter()
    }
  }
}

#[cfg(target_family = "wasm")]
pub use wasm_shims::{
  IntoParallelIterator, IntoParallelRefIterator, IntoParallelRefMutIterator, IteratorExt,
  ParallelBridge, ParallelIterator,
};

#[cfg(not(target_family = "wasm"))]
pub use rayon::iter::{
  IndexedParallelIterator, IntoParallelIterator, IntoParallelRefIterator,
  IntoParallelRefMutIterator, ParallelBridge, ParallelIterator,
};

fn _usages() {
  let mut demo = vec![1, 2, 3, 4, 5];
  demo.iter().par_bridge().for_each(|_| {});
  demo.iter_mut().par_bridge().for_each(|_| {});
  demo.clone().into_iter().par_bridge().for_each(|_| {});
  demo.par_iter().for_each(|_| {});
  demo.clone().into_par_iter().for_each(|_| {});
}
