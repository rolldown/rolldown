import { errrorText, runCommand, successText } from './utils.js';
import chokidar from 'chokidar';

/**
 * 
 * @param {string} path 
 * @returns string
 */

function getComponentName(path) {
    return path.split('/')[1];
}
chokidar
    .watch('./crates')
    .on('ready', () => console.log('Initial scan complete. Ready for changes'))
    .on('change', (path) => {
        console.log(path)
        const crate = getComponentName(path);
        console.log(`${crate} changed,now rebuilding.....`);
        runCommand(`npm run build:binding`, `./`, {
            silent: false
        }).then(res => {
            console.log(successText('Success'), 'rebuild over')
        }).catch(err => {
            console.log(errrorText('Error'), err)
        })
    }); 