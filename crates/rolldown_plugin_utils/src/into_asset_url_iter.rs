use std::ops::Range;

pub struct AssetUrlIter<'a>(&'a str);

#[derive(Debug)]
pub enum AssetUrlItem<'a> {
  Asset((Range<usize>, &'a str, Option<&'a str>)),
  PublicAsset((Range<usize>, &'a str)),
}

impl<'a> From<&'a str> for AssetUrlIter<'a> {
  fn from(code: &'a str) -> Self {
    AssetUrlIter(code)
  }
}

impl<'a> AssetUrlIter<'a> {
  pub fn into_asset_url_iter(&self) -> impl Iterator<Item = AssetUrlItem<'a>> {
    self.0.match_indices("__VITE_ASSET_").filter_map(|(start, _)| {
      if self.0[start + 13..].starts_with('_') {
        let hash_s = start + 14;
        let hash_e = hash_s + self.0[hash_s..].find("__")?;
        let reference_id = &self.0[hash_s..hash_e];
        let mut end = reference_id
          .bytes()
          .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'$')
          .then_some(hash_e + 2)?;
        let postfix = self.0[end..].starts_with("$_").then(|| {
          self.0[end + 2..].find("__").map_or("", |i| {
            let v = &self.0[end + 2..end + 2 + i];
            end = end + 2 + i + 2;
            v
          })
        });
        return Some(AssetUrlItem::Asset((start..end, reference_id, postfix)));
      }
      if self.0[start + 13..].starts_with("PUBLIC__") {
        let hash_s = start + 21;
        let hash_e = hash_s + 8;
        let hash = self.0.get(hash_s..hash_e)?;
        if self.0.get(hash_e..hash_e + 2)? != "__" {
          return None;
        }
        return hash
          .bytes()
          .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
          .then_some(AssetUrlItem::PublicAsset((start..hash_e + 2, hash)));
      }
      None
    })
  }
}

#[test]
fn test_into_asset_url_iter() {
  let result: Vec<_> = AssetUrlIter::from(
    "__VITE_ASSET__adwA22929_$__;__VITE_ASSET__333__$_?666__;__VITE_ASSET_PUBLIC__12345678__",
  )
  .into_asset_url_iter()
  .map(|item| match item {
    AssetUrlItem::Asset((range, hash, postfix)) => (range, hash, postfix),
    AssetUrlItem::PublicAsset((range, hash)) => (range, hash, None),
  })
  .collect();
  assert_eq!(
    result,
    vec![(0..27, "adwA22929_$", None), (28..55, "333", Some("?666")), (56..87, "12345678", None)]
  );
}
