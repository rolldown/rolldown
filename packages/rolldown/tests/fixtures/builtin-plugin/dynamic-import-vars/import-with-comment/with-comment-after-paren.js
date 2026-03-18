export function withCommentAfterParen(name) {
  return import(/* comment */ `./dir/${name}.js`);
}
