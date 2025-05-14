use memchr::{memchr, memchr2};

#[inline]
/// ref https://github.com/rolldown/vite/blob/454c8fff9f7115ed29281c2d927366280508a0ab/packages/vite/src/shared/utils.ts#L31-L34
/// https://regex101.com/delete/E5Xk8cGCIde8tiY8I4TOe9eWqgTxyQj006TK
pub fn clean_url(v: &str) -> &str {
  if let Some(index) = memchr2(b'?', b'#', v.as_bytes()) { &v[0..index] } else { v }
}

pub fn get_query(v: &str) -> &str {
  if let Some(index) = memchr(b'?', v.as_bytes()) { &v[index + 1..] } else { "" }
}
