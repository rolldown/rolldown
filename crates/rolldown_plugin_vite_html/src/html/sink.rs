use std::{
  cell::{Cell, RefCell},
  rc::{Rc, Weak},
};

use html5gum::Span;
use string_cache::DefaultAtom as Atom;

/// The different kinds of nodes in the DOM.
#[derive(Debug)]
pub enum NodeData {
  /// The `Document` itself - the root node of a HTML document.
  Document,

  /// A DOCTYPE declaration.
  Doctype,

  /// A text node.
  Text { contents: String, span: Span },

  /// A comment.
  Comment,

  /// An element with attributes.
  Element {
    /// Tag name (e.g., "div", "script") - using Atom for interning
    name: Atom,
    /// Attributes of the element
    attrs: RefCell<Vec<Attribute>>,
    /// Source position of this element
    span: Span,
  },
}

/// HTML attribute with span information
#[derive(Debug, Clone)]
pub struct Attribute {
  /// Attribute name (e.g., "class", "id") - using Atom for interning
  pub name: Atom,
  /// Attribute value - kept as String since values are usually unique
  pub value: String,
  /// Source position of this attribute
  pub span: Span,
}

/// A DOM node.
pub struct Node {
  /// Parent node.
  pub parent: Cell<Option<WeakHandle>>,
  /// Child nodes of this node.
  pub children: RefCell<Vec<Handle>>,
  /// Represents this node's data.
  pub data: NodeData,
}

impl std::fmt::Debug for Node {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Node")
      .field("data", &self.data)
      .field("parent", &"<parent>")
      .field("children", &self.children)
      .finish()
  }
}

impl Node {
  /// Create a new node from its contents
  pub fn new(data: NodeData) -> Rc<Self> {
    Rc::new(Node { data, parent: Cell::new(None), children: RefCell::new(Vec::new()) })
  }
}

/// Reference to a DOM node.
pub type Handle = Rc<Node>;

/// Weak reference to a DOM node, used for parent pointers.
pub type WeakHandle = Weak<Node>;

/// Append a parentless node to another nodes' children
pub fn append(new_parent: &Handle, child: Handle) {
  let previous_parent = child.parent.replace(Some(Rc::downgrade(new_parent)));
  // Invariant: child cannot have existing parent
  assert!(previous_parent.is_none());
  new_parent.children.borrow_mut().push(child);
}

/// The DOM itself; the result of parsing.
#[derive(Debug)]
pub struct RcDom {
  /// The `Document` itself.
  pub document: Handle,
  /// Errors that occurred during parsing.
  pub errors: RefCell<Vec<&'static str>>,
}

impl Default for RcDom {
  fn default() -> RcDom {
    RcDom { document: Node::new(NodeData::Document), errors: RefCell::default() }
  }
}

/// DOM builder that processes tokens and builds the RcDom
pub struct RcDomEmitter {
  pub dom: RcDom,
  current_node: Handle,
  open_elements: Vec<Handle>,
}

impl RcDomEmitter {
  pub fn new() -> Self {
    let dom = RcDom::default();
    let document = Rc::clone(&dom.document);
    Self { dom, current_node: Rc::clone(&document), open_elements: vec![document] }
  }

  pub fn finish(self) -> RcDom {
    self.dom
  }

  pub fn add_element(&mut self, name: Atom, attrs: Vec<Attribute>, span: Span, self_closing: bool) {
    let element = Node::new(NodeData::Element { name, attrs: RefCell::new(attrs), span });

    append(&self.current_node, Rc::clone(&element));

    if !self_closing {
      self.open_elements.push(Rc::clone(&element));
      self.current_node = element;
    }
  }

  pub fn close_element(&mut self, name: &str) {
    // Find the matching open element and close it
    for i in (0..self.open_elements.len()).rev() {
      if let NodeData::Element { name: ref elem_name, .. } = self.open_elements[i].data {
        if elem_name == name {
          self.open_elements.truncate(i);
          if !self.open_elements.is_empty() {
            self.current_node = Rc::clone(self.open_elements.last().unwrap());
          }
          break;
        }
      }
    }
  }

  pub fn add_text(&self, contents: String, span: Span) {
    let text_node = Node::new(NodeData::Text { contents, span });
    append(&self.current_node, text_node);
  }

  pub fn add_comment(&self) {
    let comment_node = Node::new(NodeData::Comment);
    append(&self.current_node, comment_node);
  }

  pub fn add_doctype(&self) {
    let doctype_node = Node::new(NodeData::Doctype);
    append(&self.current_node, doctype_node);
  }

  pub fn add_parse_error(&self, error: &'static str) {
    self.dom.errors.borrow_mut().push(error);
  }
}
