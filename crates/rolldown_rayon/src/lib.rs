#[cfg(not(feature = "rayon"))]
pub use std::iter::Iterator as ParallelIterator;

#[cfg(not(feature = "rayon"))]
pub trait ParallelBridge: Sized {
  fn par_bridge(self) -> Self;
}

#[cfg(not(feature = "rayon"))]
impl<T: Iterator + Send> ParallelBridge for T {
  fn par_bridge(self) -> Self {
    self
  }
}

#[cfg(not(feature = "rayon"))]
pub trait IntoParallelIterator: Sized {
  type Item;
  type Iter: Iterator<Item = Self::Item>;

  fn into_par_iter(self) -> Self::Iter;
}

#[cfg(not(feature = "rayon"))]
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

#[cfg(not(feature = "rayon"))]
pub trait IntoParallelRefIterator<'data> {
  type Item: 'data;
  type Iter: ParallelIterator<Item = Self::Item>;

  fn par_iter(&'data self) -> Self::Iter;
}

#[cfg(not(feature = "rayon"))]
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

#[cfg(feature = "rayon")]
pub use rayon::iter::{
  IntoParallelIterator, IntoParallelRefIterator, ParallelBridge, ParallelIterator,
};
