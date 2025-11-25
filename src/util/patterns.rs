use anyhow::Result;
use regex::RegexSet;
use serde::Deserialize;
use std::collections::HashMap;

/// 配置模式集合，包含原始模式和编译后的正则表达式
#[derive(Debug)]
pub struct PatternSet {
  /// 编译后的正则
  compiled_regex: Option<RegexSet>,
}

impl PatternSet {
  /// 创建新的PatternSet并编译正则表达式
  pub fn new(string_patterns: Vec<String>) -> Result<Self> {
    let compiled_regex = if string_patterns.is_empty() {
      None
    } else {
      Some(RegexSet::new(&string_patterns)?)
    };

    Ok(Self { compiled_regex })
  }

  /// 获取正则
  pub fn get_regex(&self) -> Option<&RegexSet> {
    self.compiled_regex.as_ref()
  }
}

/// 完整的模式配置，按文件扩展名组织
#[derive(Debug)]
pub struct PatternConfig {
  patterns: HashMap<String, PatternSet>,
}

impl PatternConfig {
  /// 从原始配置创建PatternConfig
  pub fn from_raw_config(raw_config: RawPatternConfig) -> Result<Self> {
    let mut patterns = HashMap::new();
    for (file_extension, patterns_vec) in raw_config.patterns {
      patterns.insert(file_extension, PatternSet::new(patterns_vec)?);
    }
    Ok(Self { patterns })
  }

  /// 检查是否包含指定扩展名
  pub fn contains_extension(&self, ext: &str) -> bool {
    self.patterns.contains_key(ext)
  }

  /// 获取指定扩展名的模式集合
  pub fn get_pattern_set(&self, ext: &str) -> Option<&PatternSet> {
    self.patterns.get(ext)
  }
}

#[derive(Debug, Deserialize)]
pub struct RawPatternConfig {
  #[serde(flatten)]
  pub patterns: HashMap<String, Vec<String>>,
}
