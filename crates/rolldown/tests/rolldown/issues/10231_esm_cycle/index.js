import A from './a.js';
export default function run() {
  return new A().make().constructor.name;
}
