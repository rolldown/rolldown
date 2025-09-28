use html5gum::{DefaultEmitter, Token, Tokenizer};
use string_cache::DefaultAtom as Atom;

use super::sink::{Attribute, RcDom, RcDomEmitter};

pub fn parse_html(html: &str) -> RcDom {
  let mut dom_builder = RcDomEmitter::new();
  let tokenizer = Tokenizer::new_with_emitter(html, DefaultEmitter::<usize>::new_with_span());

  for token in tokenizer {
    match token {
      Ok(Token::StartTag(tag)) => {
        let mut attrs = Vec::with_capacity(tag.attributes.len());

        for (attr_name, attr_value) in tag.attributes {
          let name = Atom::from(unsafe { String::from_utf8_unchecked(attr_name.0) });
          let value = unsafe { String::from_utf8_unchecked(attr_value.value.0) };
          attrs.push(Attribute { name, value, span: attr_value.span });
        }

        dom_builder.add_element(
          Atom::from(String::from_utf8_lossy(&tag.name)),
          attrs,
          tag.self_closing,
        );
      }
      Ok(Token::EndTag(tag)) => {
        let name = unsafe { String::from_utf8_unchecked(tag.name.0) };
        dom_builder.close_element(&name);
      }
      Ok(Token::String(s)) => {
        let contents = unsafe { String::from_utf8_unchecked(s.0.clone()) };
        dom_builder.add_text(contents, s.span);
      }
      Ok(Token::Comment(_)) => {
        dom_builder.add_comment();
      }
      Ok(Token::Doctype(_)) => {
        dom_builder.add_doctype();
      }
      Ok(Token::Error(e)) => {
        dom_builder.add_parse_error(e.as_str());
      }
      Err(_) => {
        break;
      }
    }
  }

  dom_builder.finish()
}
