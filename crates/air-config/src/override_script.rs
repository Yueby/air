use rquickjs::{Context, Runtime};

use air_config::model::MihomoConfigDocument;
use air_error::ConfigError;

pub const DEFAULT_OVERRIDE_SCRIPT: &str = r#"function override(subscriptionName, config) {
  return config;
}
"#;

const JS_MEMORY_LIMIT_BYTES: usize = 16 * 1024 * 1024;

pub fn apply_override_script(
    subscription_name: &str,
    document: &MihomoConfigDocument,
    script: &str,
) -> Result<MihomoConfigDocument, ConfigError> {
    let source = normalized_script(script);
    let config_json = serde_json::to_string(document).map_err(json_error)?;
    let script_json = serde_json::to_string(source).map_err(json_error)?;
    let name_json = serde_json::to_string(subscription_name).map_err(json_error)?;
    let runner = override_runner(&script_json, &name_json, &config_json);
    let runtime = Runtime::new()
        .map_err(|error| ConfigError::InvalidDocument(format!("初始化 QuickJS 失败: {error}")))?;
    runtime.set_memory_limit(JS_MEMORY_LIMIT_BYTES);
    let context = Context::full(&runtime)
        .map_err(|error| ConfigError::InvalidDocument(format!("初始化 QuickJS 失败: {error}")))?;
    let output = context
        .with(|ctx| ctx.eval::<String, _>(runner.as_str()))
        .map_err(|error| ConfigError::InvalidDocument(format!("执行覆写脚本失败: {error}")))?;
    let value: serde_json::Value = serde_json::from_str(&output).map_err(json_error)?;
    if !value.is_object() {
        return Err(ConfigError::InvalidDocument(
            "覆写脚本返回值必须是 core.runtime.config.yaml 对应的 JSON 对象".into(),
        ));
    }
    serde_json::from_value(value).map_err(json_error)
}

pub fn normalized_script(script: &str) -> &str {
    if script.trim().is_empty() {
        DEFAULT_OVERRIDE_SCRIPT
    } else {
        script
    }
}

fn override_runner(script_json: &str, name_json: &str, config_json: &str) -> String {
    // 用户脚本只拿到订阅名和运行配置对象；本地文件、网络和 Air 内部状态都不暴露给 QuickJS。
    // 支持两种写法：直接写 function 表达式，或声明名为 override 的函数。
    format!(
        r#"
"use strict";
const __air_source = {script_json};
const __air_config = {config_json};
let __air_override = undefined;
try {{
  __air_override = eval("(" + __air_source + ")");
}} catch (__air_expression_error) {{
  eval(__air_source);
  if (typeof override === "function") {{
    __air_override = override;
  }}
}}
if (typeof __air_override !== "function") {{
  throw new Error("脚本需要导出一个函数，或声明 function override(subscriptionName, config)");
}}
const __air_result = __air_override({name_json}, __air_config);
if (__air_result === undefined) {{
  JSON.stringify(__air_config);
}} else {{
  JSON.stringify(__air_result);
}}
"#
    )
}

fn json_error(error: serde_json::Error) -> ConfigError {
    ConfigError::InvalidDocument(format!("覆写脚本 JSON 转换失败: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use air_config::ConfigDocument;

    #[test]
    fn override_script_can_mutate_runtime_config_object() {
        let document = ConfigDocument::parse("mixed-port: 7890\nrules: []\n")
            .unwrap()
            .typed;
        let script = r#"
function override(subscriptionName, config) {
  config["mixed-port"] = subscriptionName === "Work" ? 19090 : 7890;
  config.rules.push("MATCH,DIRECT");
  return config;
}
"#;

        let result = apply_override_script("Work", &document, script).unwrap();

        assert_eq!(result.global.mixed_port, Some(19090));
        assert_eq!(result.rules[0].raw, "MATCH,DIRECT");
    }

    #[test]
    fn empty_script_keeps_config_unchanged() {
        let document = ConfigDocument::parse("mixed-port: 7890\n").unwrap().typed;

        let result = apply_override_script("Local", &document, "").unwrap();

        assert_eq!(result.global.mixed_port, Some(7890));
    }

    #[test]
    fn non_object_result_is_rejected() {
        let document = ConfigDocument::parse("mixed-port: 7890\n").unwrap().typed;

        let error = apply_override_script("Local", &document, "function override() { return 1; }")
            .unwrap_err();

        assert!(matches!(error, ConfigError::InvalidDocument(_)));
    }
}
