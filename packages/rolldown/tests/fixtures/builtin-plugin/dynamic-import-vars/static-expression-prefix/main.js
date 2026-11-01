export let calls = 0;

function next(name) {
  calls++;
  return name;
}

export const loaders = [
  (name) => import(`./dir/${next(name)}.js`),
  (name) => import(`${'./dir/'}${next(name)}.js`),
  (name) => import(`./${'dir/'}${next(name)}.js`),
  (name) => import(`${'./'}${'dir/'}${next(name)}.js`),
  (name) => import(`${`./dir/${next(name)}`}.js`),
  (name) => import(`${'./dir/' + next(name)}.js`),
  (name) => import(`${'./dir/'.concat(next(name))}.js`),
  (name) => import(`${'./dir/../dir/'}${next(name)}.js`),
  (name) => import(`@/${next(name)}.js`),
  (name) => import(`${'@/'}${next(name)}.js`),
  (name) => import(`${'@😀/'}${next(name)}.js`),
];
