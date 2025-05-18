function MyDecorator(): ClassDecorator {
  return (_target) => {}
}

@MyDecorator()
class MyClass { 
  static myStaticName = MyClass.name;
}

export const staticName = MyClass.myStaticName;
