function dec(value, context) {
  context.addInitializer(function () {
    this.decorated = context.name;
  });
  return value;
}

export class C {
  @dec
  m() {}
}
