// test case copy from https://www.typescriptlang.org/tsconfig/#emitDecoratorMetadata
function LogMethod(
  target: any,
  propertyKey: string | symbol,
  descriptor: PropertyDescriptor
) {
  console.log(target);
  console.log(propertyKey);
  console.log(descriptor);
}
 
class Demo {
  @LogMethod
  public foo(bar: number) {
    // do nothing
  }
}
 
const demo = new Demo();
