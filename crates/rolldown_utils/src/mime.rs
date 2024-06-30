use infer::get;
use mime::Mime;
use mime_guess::from_ext;
use std::str::FromStr;
pub fn get_data_url_mime_by_extension(extension: &str) -> Option<Mime> {
  let mime = from_ext(extension).first_or_octet_stream();
  if valid_mime_for_data_url(&mime) {
    Some(mime)
  } else {
    None
  }
}

pub fn get_data_url_mime_by_data(data: &[u8]) -> Option<Mime> {
  let mime = get(data).expect("Failed to infer mime type from data.").mime_type();
  let mime = Mime::from_str(&mime).expect("Failed to parse mime type.");
  if valid_mime_for_data_url(&mime) {
    Some(mime)
  } else {
    None
  }
}

fn valid_mime_for_data_url(mime: &Mime) -> bool {
  matches!(
    mime.type_(),
    mime::IMAGE | mime::AUDIO | mime::VIDEO | mime::FONT | mime::TEXT | mime::APPLICATION
  )
}
