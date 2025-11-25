use std::{
  collections::HashSet,
  fs,
  path::{Path, PathBuf},
  time::Instant,
};

use anyhow::{Context, Result};
use indexmap::IndexMap;
use walkdir::WalkDir;

use crate::{
  cmd::shared::{DEFAULT_DIR_CONFIG, DEFAULT_REGEX_CONFIG},
  util::{
    json_patch::{self, PatchData},
    json5,
    patterns::{PatternConfig, RawPatternConfig},
  },
};

/// 配置文件来源，仅用于提示信息
#[derive(Debug, PartialEq)]
enum ConfigSource {
  BuiltIn,
  External,
}

/// 运行生成JSON Patch即语言模板（Language Template）的命令
pub fn run(input: PathBuf, output: PathBuf, test: bool) -> Result<()> {
  // 1. 初始部分
  // 计时开始
  let start_time = Instant::now();
  // 输入的Neki Mod本体目录
  let input_dir = input.as_path();
  // 输出的JSON Patch即语言模板目录
  let output_dir = output.as_path();
  // 是否生成test operation
  let gen_test = test;

  // 输入文件的 map
  let mut input_files_map = IndexMap::new();
  // 输出文件的 map
  let mut output_files_map = IndexMap::new();
  // 加载配置文件（文件夹白名单+正则表达式）
  let (dir_whitelist, regex_config) = load_config()?;

  // 2. 遍历输入目录
  for entry in WalkDir::new(input_dir)
    .into_iter()
    .filter_map(|e| e.ok()) // 过滤掉错误项
    .filter(|e| {
      // 过滤掉非文件项
      if !e.file_type().is_file() {
        return false;
      }
      // 过滤掉非白名单内的子目录
      let file_path = e.path();
      let relative_path = file_path.strip_prefix(input_dir).unwrap();
      if !dir_whitelist
        .iter()
        .any(|dir| relative_path.starts_with(dir))
      {
        return false;
      }
      // 过滤掉非白名单内的文件后缀名
      let (ext, _) = get_extension_info(file_path);
      regex_config.contains_extension(&ext)
    })
  {
    let file_path = entry.path();
    let (ext, is_patch) = get_extension_info(file_path);
    let json_str = fs::read_to_string(file_path)?;
    input_files_map.insert(file_path.to_path_buf(), (json_str, ext, is_patch));
  }

  let duration = start_time.elapsed();
  println!(
    "[INFO] Files reading completed - time elapsed: {}.{:03}s",
    duration.as_secs(),
    duration.subsec_millis()
  );

  // 3. 生成 patch
  for (file_path, (json_str, ext, is_patch)) in input_files_map {
    let json_value = json5::parse(&json_str)?;
    // 生成 patch
    let json_value_vec =
      json_patch::generate_patch(is_patch, &json_value, &ext, &regex_config, gen_test);
    if json_value_vec.is_empty() {
      continue;
    }
    // 输出文件名
    let output_file_path = if is_patch {
      PathBuf::from(output_dir).join(file_path.strip_prefix(input_dir)?)
    } else {
      PathBuf::from(output_dir).join(format!(
        "{}.patch",
        file_path.strip_prefix(input_dir)?.to_string_lossy()
      ))
    };
    // 写入到用于输出文件的map中
    output_files_map.insert(output_file_path, json_value_vec);
  }

  let duration = start_time.elapsed();
  println!(
    "[INFO] Patches generation completed - time elapsed: {}.{:03}s",
    duration.as_secs(),
    duration.subsec_millis()
  );

  // 4. 输出 patch 到目录
  for (output_file_path, json_value_vec) in output_files_map {
    fs::create_dir_all(
      output_file_path
        .parent()
        .context("[ERROR] Failed to get parent directory!")?,
    )?;

    match json_value_vec {
      PatchData::CommonPatch(values) => {
        fs::write(output_file_path, serde_json::to_string_pretty(&values)?)?
      }
      PatchData::BatchesPatch(values) => {
        fs::write(output_file_path, serde_json::to_string_pretty(&values)?)?
      }
    }
  }

  let duration = start_time.elapsed();
  println!(
    "[INFO] Patches writing completed - total time: {}.{:03}s",
    duration.as_secs(),
    duration.subsec_millis()
  );

  Ok(())
}

/// 加载配置
fn load_config() -> Result<(HashSet<String>, PatternConfig)> {
  // 尝试从可执行文件目录加载，如果有任何一步失败，直接使用默认配置
  let exe_dir = std::env::current_exe();

  let (dirs_str, dirs_source);
  let (regex_str, regex_source);

  // 如果可执行文件目录存在，则尝试从该目录加载配置
  // 如果可执行文件目录不存在或出现其他问题，则使用默认配置
  match exe_dir {
    Ok(o) => match o.parent() {
      Some(parent) => {
        (dirs_str, dirs_source) = read_config_file(
          Path::new(parent).join("dirs_config.json").as_path(),
          DEFAULT_DIR_CONFIG,
        )?;
        (regex_str, regex_source) = read_config_file(
          Path::new(parent).join("regex_config.json").as_path(),
          DEFAULT_REGEX_CONFIG,
        )?;
      }
      None => {
        (dirs_str, dirs_source) = (DEFAULT_DIR_CONFIG.to_owned(), ConfigSource::BuiltIn);
        (regex_str, regex_source) = (DEFAULT_REGEX_CONFIG.to_owned(), ConfigSource::BuiltIn);
      }
    },
    Err(_) => {
      (dirs_str, dirs_source) = (DEFAULT_DIR_CONFIG.to_owned(), ConfigSource::BuiltIn);
      (regex_str, regex_source) = (DEFAULT_REGEX_CONFIG.to_owned(), ConfigSource::BuiltIn);
    }
  }

  let config_msg = match (dirs_source, regex_source) {
    (ConfigSource::BuiltIn, ConfigSource::BuiltIn) => "Using built-in configurations",
    (ConfigSource::External, ConfigSource::External) => "Using external configurations",
    (ConfigSource::BuiltIn, ConfigSource::External) => {
      "Using built-in dir whitelist and external regex config"
    }
    (ConfigSource::External, ConfigSource::BuiltIn) => {
      "Using external dir whitelist and built-in regex config"
    }
  };
  println!("[INFO] {}", config_msg);

  // 解析文件夹白名单
  let dirs_value =
    json5::parse(&dirs_str).context("[ERROR] Failed to parse dir whitelist config!")?;
  let dirs = serde_json::from_value::<HashSet<String>>(dirs_value)
    .context("[ERROR] Failed to deserialize dir whitelist!")?;
  // 解析正则表达式配置
  let patterns_value = json5::parse(&regex_str).context("Failed to parse regex config!")?;
  let patterns = serde_json::from_value::<RawPatternConfig>(patterns_value)
    .context("[ERROR] Failed to deserialize regex config!")?;
  let patterns_regex = PatternConfig::from_raw_config(patterns)?;

  Ok((dirs, patterns_regex))
}

/// 读取配置文件内容，返回内容和来源
fn read_config_file(path: &Path, default: &str) -> Result<(String, ConfigSource)> {
  // 如果文件不存在，则使用默认配置
  if !path.exists() {
    return Ok((default.to_owned(), ConfigSource::BuiltIn));
  }
  // 读取文件内容
  let content = fs::read_to_string(path).context("[ERROR] Failed to read config file!")?;

  Ok((content, ConfigSource::External))
}

/// 获取文件扩展名信息
fn get_extension_info(file_path: &Path) -> (String, bool) {
  // 无后缀名时返回空字符串
  let mut file_extension = file_path
    .extension()
    .and_then(|s| s.to_str())
    .unwrap_or("")
    .to_string();

  // 特殊处理patch文件
  let is_patch = file_extension == "patch";
  // 如果是patch文件，则获取上一级后缀名，并拼接成完整后缀名
  if is_patch {
    if let Some(file_stem) = file_path.file_stem().and_then(|s| s.to_str()) {
      if let Some(char_index) = file_stem.rfind('.') {
        // 拼接成完整后缀名，如 example.patch
        file_extension = format!("{}.{file_extension}", &file_stem[char_index + 1..]);
      }
    }
  }

  (file_extension, is_patch)
}
