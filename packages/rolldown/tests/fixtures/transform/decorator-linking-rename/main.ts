function MyDecorator(): ClassDecorator {
  return (target) => {}
}

@MyDecorator()
class MyClass { 
  myName = MyClass.name;
}

export const myName = new MyClass().myName;
