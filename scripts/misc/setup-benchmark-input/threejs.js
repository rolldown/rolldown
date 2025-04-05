import fsExtra from 'fs-extra';
// setup three-js

if (fsExtra.existsSync('./tmp/bench/three')) {
  console.log('[skip] setup three already');
} else {
  console.log('Setup `three` in tmp/bench');
  fsExtra.copySync('./tmp/github/three', `./tmp/bench/three`);

  fsExtra.writeFileSync(
    './tmp/bench/three/entry.js',
    `import * as three from './src/Three.js'; export { three }`,
  );
}
