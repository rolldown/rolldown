#![allow(dead_code)]

use std::rc::Rc;

use super::sink::{Handle, NodeData, RcDom};

/// Trait for visiting nodes in the DOM tree
pub trait DomVisitor {
  /// Visit a document node
  fn visit_document(&mut self, _node: &Handle) {}

  /// Visit a doctype node
  fn visit_doctype(&mut self, _node: &Handle) {}

  /// Visit a text node
  fn visit_text(&mut self, _node: &Handle) {}

  /// Visit a comment node
  fn visit_comment(&mut self, _node: &Handle) {}

  /// Visit an element node (called before visiting children)
  fn visit_element_start(&mut self, _node: &Handle, _name: &str) {}

  /// Visit an element node (called after visiting children)
  fn visit_element_end(&mut self, _node: &Handle, _name: &str) {}
}

/// Walk the DOM tree with a visitor
pub fn walk_dom<V: DomVisitor>(dom: &RcDom, visitor: &mut V) {
  walk_node(&dom.document, visitor);
}

/// Walk a single node and its children
pub fn walk_node<V: DomVisitor>(node: &Handle, visitor: &mut V) {
  match &node.data {
    NodeData::Document => {
      visitor.visit_document(node);
      for child in node.children.borrow().iter() {
        walk_node(child, visitor);
      }
    }
    NodeData::Doctype => {
      visitor.visit_doctype(node);
    }
    NodeData::Text { .. } => {
      visitor.visit_text(node);
    }
    NodeData::Comment => {
      visitor.visit_comment(node);
    }
    NodeData::Element { name, .. } => {
      // Atom implements Deref<Target = str>
      visitor.visit_element_start(node, name);
      for child in node.children.borrow().iter() {
        walk_node(child, visitor);
      }
      visitor.visit_element_end(node, name);
    }
  }
}

/// Collector that finds all elements matching a condition
pub struct ElementCollector<F> {
  pub elements: Vec<Handle>,
  predicate: F,
}

impl<F> ElementCollector<F>
where
  F: FnMut(&str) -> bool,
{
  pub fn new(predicate: F) -> Self {
    ElementCollector { elements: Vec::new(), predicate }
  }
}

impl<F> DomVisitor for ElementCollector<F>
where
  F: FnMut(&str) -> bool,
{
  fn visit_element_start(&mut self, node: &Handle, name: &str) {
    if (self.predicate)(name) {
      self.elements.push(Rc::clone(node));
    }
  }
}
