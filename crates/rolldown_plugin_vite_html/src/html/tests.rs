#[cfg(test)]
mod parser_tests {
  use std::rc::Rc;

  use crate::html::visitor::{ElementCollector, walk_dom};

  use crate::html::parser::parse_html;
  use crate::html::sink::{Handle, NodeData, RcDom};

  fn find_elements_by_tag(dom: &RcDom, tag_name: &str) -> Vec<Handle> {
    let mut collector = ElementCollector::new(|name| name == tag_name);
    walk_dom(dom, &mut collector);
    collector.elements
  }

  #[test]
  fn test_parse_simple_html() {
    let html = "<!DOCTYPE html>
<html>
  <head>
    <title>Test Page</title>
  </head>
  <body>
    <h1>Hello World</h1>
    <p>This is a test.</p>
  </body>
</html>";

    let dom = parse_html(html);
    assert_eq!(dom.errors.borrow().len(), 0);

    // Check that document node exists
    assert!(matches!(dom.document.data, NodeData::Document));

    // Should have at least one child (html element)
    assert!(!dom.document.children.borrow().is_empty());
  }

  #[test]
  fn test_parse_with_scripts() {
    let html = r#"<!DOCTYPE html>
<html>
  <head>
    <script src="app.js"></script>
    <script type="module">
      console.log("Hello");
    </script>
  </head>
</html>"#;

    let dom = parse_html(html);
    let scripts = find_elements_by_tag(&dom, "script");

    assert_eq!(scripts.len(), 2);

    // Check first script has src attribute
    if let NodeData::Element { ref attrs, .. } = scripts[0].data {
      let attrs = attrs.borrow();
      assert!(attrs.iter().any(|a| &*a.name == "src" && a.value == "app.js"));
    }

    // Check second script has type="module"
    if let NodeData::Element { ref attrs, .. } = scripts[1].data {
      let attrs = attrs.borrow();
      assert!(attrs.iter().any(|a| &*a.name == "type" && a.value == "module"));
    }
  }

  #[test]
  fn test_self_closing_tags() {
    fn count_elements(node: &Handle, count: &mut usize) {
      if let NodeData::Element { ref name, .. } = node.data {
        if matches!(&**name, "img" | "br" | "input") {
          *count += 1;
        }
      }
      for child in node.children.borrow().iter() {
        count_elements(child, count);
      }
    }

    let html = r#"<img src="test.jpg" /><br><input type="text">"#;
    let dom = parse_html(html);

    let mut self_closing_count = 0;

    count_elements(&dom.document, &mut self_closing_count);
    assert_eq!(self_closing_count, 3);
  }

  #[test]
  fn test_parse_with_errors() {
    let html = "<div><span></div></span>"; // Mismatched tags

    let dom = parse_html(html);

    // Even with errors, parsing should complete
    assert!(!dom.document.children.borrow().is_empty());
  }

  #[test]
  fn test_nested_elements() {
    fn count_all_elements(node: &Handle, count: &mut usize) {
      if matches!(node.data, NodeData::Element { .. }) {
        *count += 1;
      }
      for child in node.children.borrow().iter() {
        count_all_elements(child, count);
      }
    }

    let html = "
      <div>
        <ul>
          <li>Item 1</li>
          <li>Item 2
            <ul>
              <li>Nested</li>
            </ul>
          </li>
        </ul>
      </div>
    ";

    let dom = parse_html(html);

    // Count total elements
    let mut element_count = 0;

    count_all_elements(&dom.document, &mut element_count);
    assert!(element_count >= 6); // div, ul, li, li, ul, li
  }

  #[test]
  fn test_attribute_values() {
    fn find_element(node: &Handle) -> Option<Handle> {
      if matches!(node.data, NodeData::Element { .. }) {
        return Some(Rc::clone(node));
      }
      for child in node.children.borrow().iter() {
        if let Some(found) = find_element(child) {
          return Some(found);
        }
      }
      None
    }

    let html = r#"<div id="myId" class="class1 class2" data-value="123"></div>"#;
    let dom = parse_html(html);

    if let Some(div) = find_element(&dom.document) {
      if let NodeData::Element { ref attrs, .. } = div.data {
        let attrs = attrs.borrow();
        assert_eq!(attrs.len(), 3);

        assert!(attrs.iter().any(|a| &*a.name == "id" && a.value == "myId"));
        assert!(attrs.iter().any(|a| &*a.name == "class" && a.value == "class1 class2"));
        assert!(attrs.iter().any(|a| &*a.name == "data-value" && a.value == "123"));
      }
    }
  }
}
