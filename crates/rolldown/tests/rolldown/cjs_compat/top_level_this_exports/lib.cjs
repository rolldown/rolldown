this.answer = 42;
this.read = (exports) => this.answer;
{
  let exports = { answer: 0 };
  this.blocked = this.answer + exports.answer;
}
