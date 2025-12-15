import assert from 'node:assert'
function MyDecorator(): ClassDecorator {
  return (target) => {}
}

@MyDecorator()
class MyClass { 
  myName = MyClass.name;
}

export const myName = new MyClass().myName;


assert.strictEqual(myName, 'MyClass');
