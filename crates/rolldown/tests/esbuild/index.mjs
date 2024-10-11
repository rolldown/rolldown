import * as path from 'path'
import * as fs from 'fs'
const source = `
    test_tests__esbuild__lower__java_script_auto_accessor_es2021__config_json
    test_tests__esbuild__lower__java_script_auto_accessor_es2022__config_json
    test_tests__esbuild__lower__java_script_auto_accessor_es_next__config_json
    test_tests__esbuild__lower__java_script_decorators_bundle_issue3768__config_json
    test_tests__esbuild__lower__java_script_decorators_es_next__config_json
    test_tests__esbuild__lower__lower_async_arrow_super_es2016__config_json
    test_tests__esbuild__lower__lower_async_arrow_super_setter_es2016__config_json
    test_tests__esbuild__lower__lower_async_generator__config_json
    test_tests__esbuild__lower__lower_async_generator_no_await__config_json
    test_tests__esbuild__lower__lower_async_super_es2016_no_bundle__config_json
    test_tests__esbuild__lower__lower_async_super_es2017_no_bundle__config_json
    test_tests__esbuild__lower__lower_exponentiation_operator_no_bundle__config_json
    test_tests__esbuild__lower__lower_export_star_as_name_collision__config_json
    test_tests__esbuild__lower__lower_forbid_strict_mode_syntax__config_json
    test_tests__esbuild__lower__lower_nested_function_direct_eval__config_json
    test_tests__esbuild__lower__lower_object_spread_no_bundle__config_json
    test_tests__esbuild__lower__lower_private_super_es2021__config_json
    test_tests__esbuild__lower__lower_private_super_es2022__config_json
    test_tests__esbuild__lower__lower_static_async_arrow_super_es2016__config_json
    test_tests__esbuild__lower__lower_static_async_arrow_super_setter_es2016__config_json
    test_tests__esbuild__lower__lower_strict_mode_syntax__config_json
    test_tests__esbuild__lower__lower_template_object__config_json
    test_tests__esbuild__lower__lower_using__config_json
    test_tests__esbuild__lower__lower_using_hoisting__config_json
    test_tests__esbuild__lower__lower_using_inside_ts_namespace__config_json
    test_tests__esbuild__lower__lower_using_unsupported_async__config_json
    test_tests__esbuild__lower__lower_using_unsupported_using_and_async__config_json
    test_tests__esbuild__lower__ts_lower_object_rest2017_no_bundle__config_json
    test_tests__esbuild__lower__ts_lower_object_rest2018_no_bundle__config_json


`
const dir = path.join(import.meta.dirname, "lower")

for (let item of source.split("\n").filter(Boolean)) {
  item = item.trim().slice(28, -13);
  const caseName = path.join(dir, item, '_config.json')
  const source = fs.readFileSync(caseName, 'utf8')
  const json = JSON.parse(source)
  json['expectExecuted']  = false
  fs.writeFileSync(caseName, JSON.stringify(json, null, 2))
}
