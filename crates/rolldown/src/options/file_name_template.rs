use once_cell::sync::Lazy;
use regex::Regex;

const DEFAULT_HASH_LEN: usize = 8;
const MAX_HASH_LEN: usize = 22;

#[derive(Debug)]
pub struct FileNameTemplate {
  template: String,
}

impl FileNameTemplate {
  #[allow(dead_code)]
  pub fn new(template: String) -> Self {
    Self { template }
  }
}

impl From<String> for FileNameTemplate {
  fn from(template: String) -> Self {
    Self { template }
  }
}

#[derive(Debug, Default)]
pub struct FileNameRenderOptions<'me> {
  pub name: Option<&'me str>,
  pub hash: Option<&'me str>,
}

impl FileNameTemplate {
  pub fn render(&self, options: &FileNameRenderOptions) -> String {
    const HASH_REGEX: Lazy<Regex> =
      Lazy::new(|| Regex::new(r"\[(?<key>\w+)(:(?<len>\d+))??\]").unwrap());
    let mut tmp = self.template.clone();
    HASH_REGEX.captures_iter(tmp.clone().as_str()).for_each(|caps| {
      let key = caps.name("key").unwrap().as_str();
      let pattern = match key {
        // get 'name' value
        "name" => options.name,
        // get hash value
        "hash" => match options.hash {
          Some(hash) => {
            // get hash length
            let mut len = match caps.name("len") {
              Some(match_res) => match_res.as_str().parse::<usize>().unwrap_or(DEFAULT_HASH_LEN),
              None => DEFAULT_HASH_LEN,
            };
            if len > MAX_HASH_LEN {
              // TODO add len reset warning
              len = MAX_HASH_LEN;
            }
            if hash.len() > len {
              Some(&hash[0..len])
            } else {
              Some(hash)
            }
          }
          None => None,
        },
        _ => None,
      };

      if let Some(value) = pattern {
        tmp = tmp.replace(caps.get(0).unwrap().as_str(), value);
      } else {
        // TODO add None { key } option warning
      }
    });
    tmp
  }
}

#[test]
fn file_name_template_render() {
  let file_name_template = FileNameTemplate { template: "[name]-[hash].js".to_string() };

  let name_res_default_hash_len = file_name_template
    .render(&FileNameRenderOptions { name: Some("test"), hash: Some("123456789") });
  assert_eq!(name_res_default_hash_len, "test-12345678.js");

  let file_name_template = FileNameTemplate { template: "[name]-[hash:30].js".to_string() };
  let name_res_max_hash_len = file_name_template.render(&FileNameRenderOptions {
    name: Some("test"),
    hash: Some("012345678901234567890123456789"),
  });
  assert_eq!(name_res_max_hash_len, "test-0123456789012345678901.js");

  let file_name_template = FileNameTemplate { template: "[name]-[hash:5].js".to_string() };
  let name_res_common_hash_len = file_name_template
    .render(&FileNameRenderOptions { name: Some("test"), hash: Some("0123456789") });

  assert_eq!(name_res_common_hash_len, "test-01234.js");

  let file_name_template = FileNameTemplate { template: "[name]-[hash:5].js".to_string() };
  let name_res_none_hash = file_name_template
    .render(&FileNameRenderOptions { name: Some("test"), ..FileNameRenderOptions::default() });

  assert_eq!(name_res_none_hash, "test-[hash:5].js");
}
