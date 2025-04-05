import fsExtra from 'fs-extra';

if (fsExtra.existsSync('./tmp/bench/rolldown-benchcases')) {
  console.log('[skip] setup roll already');
} else {
  console.log('Setup `rolldown-benchcases` in tmp/bench');
  fsExtra.copySync(
    './tmp/github/rolldown-benchcases',
    `./tmp/bench/rolldown-benchcases`,
  );
}
