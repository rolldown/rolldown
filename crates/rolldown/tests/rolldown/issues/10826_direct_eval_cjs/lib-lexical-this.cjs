this.answer = 42;
function zero() {
  return { answer: 0 };
}
// `this` stays lexical inside the arrow and inside the block, so it still means the module's
// exports object even though each `exports` binding shadows the wrapper parameter of that name.
const read = (exports) => this.answer;
let blocked;
{
  let exports = zero();
  blocked = this.answer + exports.answer;
}
eval('');
module.exports = { answer: read(zero()) === blocked ? blocked : -1 };
