export function Comp(props) {
  const data = compute(props.a, props.b);
  return <div>{data}</div>;
}
