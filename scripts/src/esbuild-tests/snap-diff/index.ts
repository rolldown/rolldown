import { run } from './runner';

const args = process.argv.slice(2);
const debug = args.includes('--debug');
const verbose = args.includes('--verbose');
const caseNames: string[] = [];
run({ debug, verbose, caseNames });
