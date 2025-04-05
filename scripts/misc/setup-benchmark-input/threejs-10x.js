import fsExtra from 'fs-extra';

if (fsExtra.existsSync('./tmp/bench/three10x')) {
  console.log('[skip] setup three10x already');
} else {
  console.log('Setup `three10x` in tmp/bench');

  fsExtra.ensureDirSync('./tmp/bench/three10x');

  for (let i = 1; i <= 10; i++) {
    fsExtra.ensureDirSync(`./tmp/bench/three10x/copy${i}`);
    fsExtra.copySync('./tmp/bench/three/src', `./tmp/bench/three10x/copy${i}/`);
  }

  fsExtra.writeFileSync('./tmp/bench/three10x/entry.js', '');
  for (let i = 1; i <= 10; i++) {
    fsExtra.appendFileSync(
      './tmp/bench/three10x/entry.js',
      `import * as three${i} from './copy${i}/Three.js'; export { three${i} }\n`,
    );
  }
}
