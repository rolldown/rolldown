export function withCommentBeforeParen(name) {
  return import(/* comment */ `./dir/${name}.js`);
}
