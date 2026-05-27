import 'reflect-metadata';
function D(): PropertyDecorator {
  return () => {};
}

class Source {
  @D() laterRef!: LaterClass;
}

class LaterClass {
  tag = 'later';
}
