use std::path::Path;

pub fn get_ext_from_str(path: &str) -> Option<&str> {
    let path = Path::new(path);
    path.extension()?.to_str()
}
