// cSpell:ignore packagejson
import { run } from './runner'

const args = process.argv.slice(2)
const debug = args.includes('--debug')
const includeList = [
  'snapshots_importstar.txt',
  'snapshots_default.txt',
  'snapshots_packagejson.txt',
  'snapshots_dce.txt',
  'snapshots_splitting.txt',
  'snapshots_lower.txt',
  'snapshots_glob.txt',
  'snapshots_importstar_ts.txt',
  'snapshots_ts.txt',
]
run(includeList, debug)
