function _child(node) {
  return node.firstChild;
}
function _txt(node) {
  return node.DEAD_textContent;
}
const child = (...args) => child.impl(...args);
child.impl = _child;
const txt = (...args) => txt.impl(...args);
txt.impl = _txt;
export function main(node) {
  return child(node);
}
