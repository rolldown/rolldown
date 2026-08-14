export function withCommentBeforeParen(name) {
  // oxfmt-ignore
  return import /* comment */ (`./dir/${name}.js`);
}
