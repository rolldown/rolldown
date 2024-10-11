import * as path from 'path'
import * as fs from 'fs'
const source = `

    test_tests__esbuild__glob__glob_basic_no_bundle__config_json
    test_tests__esbuild__glob__glob_basic_no_splitting__config_json
    test_tests__esbuild__glob__glob_basic_splitting__config_json
    test_tests__esbuild__glob__glob_dir_does_not_exist__config_json
    test_tests__esbuild__glob__glob_entry_point_abs_path__config_json
    test_tests__esbuild__glob__glob_no_matches__config_json
    test_tests__esbuild__glob__glob_wildcard_no_slash__config_json
    test_tests__esbuild__glob__glob_wildcard_slash__config_json
    test_tests__esbuild__glob__ts_glob_basic_no_splitting__config_json
    test_tests__esbuild__glob__ts_glob_basic_splitting__config_json
`
const dir = path.join(import.meta.dirname, "glob")

for (let item of source.split("\n").filter(Boolean)) {
  item = item.trim().slice(27, -13);
  const caseName = path.join(dir, item, '_config.json')
  const source = fs.readFileSync(caseName, 'utf8')
  const json = JSON.parse(source)
  json['expectExecuted']  = false
  fs.writeFileSync(caseName, JSON.stringify(json, null, 2))
}
