export function Component(props) {
  return <div onClick={() => props.onClick()}>{props.text}</div>;
}
