function bound<T extends Function>(value: T, ctx: ClassMethodDecoratorContext): T {
  ctx.addInitializer(function (this: any) {
    this[ctx.name] = this[ctx.name].bind(this);
  });
  return value;
}
export class Greeter {
  name: string = 'x';
  @bound
  greet(): string {
    throw new Error('boom from greet');
  }
  accessor count: number = 0;
}
