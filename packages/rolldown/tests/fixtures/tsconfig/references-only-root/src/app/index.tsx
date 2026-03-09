import { function1 } from '1/index';

export const appValue = function1();

function appFactory(tag, props, ...children) {
  return { tag, props, children };
}

export function App() {
  return <div>{appValue}</div>;
}
