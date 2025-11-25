use regex::RegexSet;
use serde_json::{Value, json};

use crate::util::patterns::PatternConfig;

/// Starbound支持的JSON Patch类型：
/// 分别对应标准的Vec<Value> 或 Starbound特别支持的Vec<Vec<Value>>
#[derive(Debug, Clone)]
pub enum PatchData {
  CommonPatch(Vec<Value>),
  BatchesPatch(Vec<Vec<Value>>),
}

impl PatchData {
  pub fn is_empty(&self) -> bool {
    match self {
      PatchData::CommonPatch(patch_operations) => patch_operations.is_empty(),
      PatchData::BatchesPatch(patch_operations) => patch_operations.iter().all(|x| x.is_empty()),
    }
  }
}

/// 递归遍历 JSON，生成 patch 操作数组
fn gen_patch_from_json(
  json_value: &Value,
  json_pointer: String,
  regex_set: Option<&RegexSet>,
  patch_operations: &mut Vec<Value>,
) {
  match json_value {
    Value::String(string_value) => {
      if let Some(set) = regex_set {
        if set.is_match(&json_pointer) {
          // 生成 patch 操作
          patch_operations.push(json!({
            "op": "replace",
            "path": json_pointer,
            "value": format!("(T) {}", string_value)
          }));
        }
      }
    }
    Value::Array(array_value) => {
      if let Some(set) = regex_set {
        if set.is_match(&json_pointer) {
          // 生成 patch
          let new_array: Vec<Value> = array_value
            .iter()
            .map(|x| match x {
              Value::String(string_value) => Value::String(format!("(T) {}", string_value)),
              // unreachale???
              _ => x.clone(),
            })
            .collect();
          patch_operations.push(json!({
            "op": "replace",
            "path": json_pointer,
            "value": new_array
          }));
          // 不再递归数组内部
          return;
        }
      }
      // 递归数组元素
      for (index, value) in array_value.iter().enumerate() {
        let next_pointer = if json_pointer.is_empty() {
          format!("/{}", index)
        } else {
          format!("{}/{}", json_pointer, index)
        };
        gen_patch_from_json(value, next_pointer, regex_set, patch_operations);
      }
    }
    Value::Object(object_value) => {
      for (key, value) in object_value {
        let next_pointer = if json_pointer.is_empty() {
          format!("/{}", key)
        } else {
          format!("{}/{}", json_pointer, key)
        };

        gen_patch_from_json(value, next_pointer, regex_set, patch_operations);
      }
    }
    _ => {}
  }
}

/// 递归处理 JSON 数据，生成 patch 操作
/// is_patch_value: 是否处理 patch 对象中的 value 部分
fn gen_patch_from_json_patch(
  json_value: &Value,
  operation_path: &str,
  regex_set: Option<&RegexSet>,
  patch_operations: &mut Vec<Value>,
  is_patch_value: bool,
) {
  match json_value {
    Value::String(string_value) => {
      if let Some(set) = regex_set {
        if set.is_match(operation_path) {
          patch_operations.push(json!({
            "op": "replace",
            "path": operation_path,
            "value": format!("(T) {}", string_value)
          }));
        }
      }
    }
    Value::Array(array_value) => {
      if let Some(set) = regex_set {
        if set.is_match(operation_path) {
          let new_array: Vec<Value> = array_value
            .iter()
            .map(|x| match x {
              Value::String(string_value) => Value::String(format!("(T) {}", string_value)),
              _ => x.clone(),
            })
            .collect();
          patch_operations.push(json!({
            "op": "replace",
            "path": operation_path,
            "value": new_array
          }));
          return;
        }
      }
      for (i, v) in array_value.iter().enumerate() {
        let next_path = format!("{}/{}", operation_path, i);
        gen_patch_from_json_patch(v, &next_path, regex_set, patch_operations, is_patch_value);
      }
    }
    Value::Object(object_value) => {
      if !is_patch_value {
        // 处理 patch 对象
        if let (Some(Value::String(op)), Some(Value::String(path)), Some(val)) = (
          object_value.get("op"),
          object_value.get("path"),
          object_value.get("value"),
        ) {
          if op == "replace" || op == "add" {
            gen_patch_from_json_patch(val, path, regex_set, patch_operations, true);
            return;
          }
        }
      }

      // 递归处理对象字段
      for (k, v) in object_value {
        let next_path = if is_patch_value {
          format!("{}/{}", operation_path, k)
        } else {
          k.to_string()
        };
        gen_patch_from_json_patch(v, &next_path, regex_set, patch_operations, is_patch_value);
      }
    }
    _ => {}
  }
}

/// 处理JSON数据，生成从JSON本身的patch操作数组
fn process_json(
  json_value: &Value,
  regex_set: Option<&RegexSet>,
  gen_test_operation: bool,
) -> PatchData {
  let mut patch_operations = Vec::new();
  gen_patch_from_json(json_value, String::new(), regex_set, &mut patch_operations);

  if gen_test_operation {
    generate_test_operation(&patch_operations)
  } else {
    PatchData::CommonPatch(patch_operations)
  }
}

/// 处理JSON数据，生成一维patch操作数组
fn process_json_patch(
  json_value: &Value,
  regex_set: Option<&RegexSet>,
  gen_test_operation: bool,
) -> PatchData {
  let mut patch_operations = Vec::new();
  gen_patch_from_json_patch(json_value, "", regex_set, &mut patch_operations, false);

  if gen_test_operation {
    generate_test_operation(&patch_operations)
  } else {
    PatchData::CommonPatch(patch_operations)
  }
}

fn generate_test_operation(patch_operations: &Vec<Value>) -> PatchData {
  let mut patch_batch = Vec::new();

  for patch_operation in patch_operations {
    patch_batch.push(Vec::from([
      json!({
        "op": "test",
        "path": patch_operation["path"].as_str().unwrap(),
      }),
      patch_operation.clone(),
    ]));
  }

  PatchData::BatchesPatch(patch_batch)
}

/// 对外主方法：输入判断是否为JSON patch的布尔值、Value、文件后缀、PatternConfig，输出 patch 数组
pub fn generate_patch(
  is_patch: bool,
  json_value: &Value,
  file_extension: &str,
  pattern_config: &PatternConfig,
  gen_test_operation: bool,
) -> PatchData {
  match pattern_config.get_pattern_set(file_extension) {
    Some(pattern_set) => {
      if is_patch {
        process_json_patch(json_value, pattern_set.get_regex(), gen_test_operation)
      } else {
        process_json(json_value, pattern_set.get_regex(), gen_test_operation)
      }
    }
    // unreachale???
    None => PatchData::CommonPatch(Vec::new()),
  }
}
