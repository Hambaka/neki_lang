use std::{error::Error, fmt};

use serde_json::Value;

const WS: [char; 8] = [
  ' ', '\t', '\r', '\n', '\u{000B}', // \v
  '\u{000C}', // \f
  '\u{00A0}', // \xA0
  '\u{FEFF}', // \uFEFF
];

fn escapee_get(esc: char) -> Option<&'static str> {
  match esc {
    '\'' => Some("'"),
    '"' => Some("\""),
    '\\' => Some("\\"),
    '/' => Some("/"),
    '\n' => Some(""),        // 转义换行符替换为空字符串
    'b' => Some("\u{0008}"), // \b
    'f' => Some("\u{000C}"), // \f
    'n' => Some("\n"),
    'r' => Some("\r"),
    't' => Some("\t"),
    _ => None,
  }
}

#[derive(Debug)]
pub struct ParseError {
  pub message: String,
}

impl fmt::Display for ParseError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{}", self.message)
  }
}

impl Error for ParseError {}

type ParseResult<T> = Result<T, ParseError>;

pub struct Parser {
  /// The index of the current character
  at: usize,
  /// The current line number
  line_number: usize,
  /// The current column number
  column_number: usize,
  /// The current character
  ch: Option<char>,
  /// The input text，store as vector of chars for faster access
  text: Vec<char>,
}

impl Parser {
  pub fn new(input_str: &str) -> Self {
    Self {
      at: 0,
      line_number: 1,
      column_number: 1,
      ch: Some(' '),
      text: input_str.chars().collect(),
    }
  }

  fn error(&self, msg: String) -> ParseError {
    let start = self.at.saturating_sub(1);
    let end = (self.at + 19).min(self.text.len());
    let snippet: String = self.text[start..end].iter().collect();
    let snippet_json = serde_json::to_string(&snippet).unwrap();

    ParseError {
      message: format!(
        "{} at line {} column {}. Next part: {}",
        msg, self.line_number, self.column_number, snippet_json
      ),
    }
  }

  fn next(&mut self, expect: Option<char>) -> ParseResult<Option<char>> {
    // 如果有期望字符，检查当前字符是否匹配
    if let Some(c) = expect {
      if self.ch != Some(c) {
        return Err(self.error(format!(
          "Expected {} instead of {}",
          render_char(c),
          self.ch.map_or("EOF".to_string(), |x| render_char(x))
        )));
      }
    }

    // 获取下一个字符
    self.ch = self.text.get(self.at).copied();
    self.at += 1;
    self.column_number += 1;

    // 处理换行
    if let Some(ch) = self.ch {
      if ch == '\n' || (ch == '\r' && self.peek() != Some('\n')) {
        self.line_number += 1;
        self.column_number = 0;
      }
    }

    Ok(self.ch)
  }

  /// Get the next character without consuming it or
  /// assigning it to the ch varaible.
  fn peek(&self) -> Option<char> {
    self.text.get(self.at).copied()
  }

  /// Parse a number value.
  fn number(&mut self) -> ParseResult<Value> {
    let mut sign = 1.0;
    let mut string = String::new();
    let mut base = 10;
    let mut is_float = false;

    // 处理正负号
    if let Some(ch) = self.ch {
      if ch == '-' || ch == '+' {
        if ch == '-' {
          sign = -1.0;
        }
        self.next(Some(ch))?;
      }
    }

    // 处理 Infinity
    if self.ch == Some('I') {
      let val = self.word()?;
      if let Value::String(ref s) = val {
        if s == "Infinity" {
          return match serde_json::Number::from_f64(sign * f64::INFINITY) {
            Some(num) => Ok(Value::Number(num)),
            None => Err(self.error("Bad number".to_string())),
          };
        }
      }
      return Err(self.error("Unexpected word for number".to_string()));
    }

    // 处理 NaN
    if self.ch == Some('N') {
      let val = self.word()?;
      if let Value::String(ref s) = val {
        if s == "NaN" {
          return match serde_json::Number::from_f64(f64::NAN) {
            Some(num) => Ok(Value::Number(num)),
            None => Err(self.error("Bad number".to_string())),
          };
        }
      }
      return Err(self.error("expected word to be NaN".to_string()));
    }

    // 处理 0x/0X 十六进制
    if self.ch == Some('0') {
      string.push('0');
      self.next(None)?;
      if let Some(ch) = self.ch {
        if ch == 'x' || ch == 'X' {
          string.push(ch);
          self.next(None)?;
          base = 16;
        } else if ch.is_digit(10) {
          return Err(self.error("Octal literal".to_string()));
        }
      }
    }

    match base {
      10 => {
        // 整数部分
        while let Some(ch) = self.ch {
          if ch.is_digit(10) {
            string.push(ch);
            self.next(None)?;
          } else {
            break;
          }
        }
        // 小数部分
        if self.ch == Some('.') {
          is_float = true;
          string.push('.');
          self.next(None)?;
          while let Some(ch) = self.ch {
            if ch.is_digit(10) {
              string.push(ch);
              self.next(None)?;
            } else {
              break;
            }
          }
        }
        // 指数部分
        if let Some(ch) = self.ch {
          if ch == 'e' || ch == 'E' {
            is_float = true;
            string.push(ch);
            self.next(None)?;
            if let Some(ch2) = self.ch {
              if ch2 == '-' || ch2 == '+' {
                string.push(ch2);
                self.next(None)?;
              }
            }
            while let Some(ch3) = self.ch {
              if ch3.is_digit(10) {
                string.push(ch3);
                self.next(None)?;
              } else {
                break;
              }
            }
          }
        }
      }
      16 => {
        while let Some(ch) = self.ch {
          if ch.is_digit(16) {
            string.push(ch);
            self.next(None)?;
          } else {
            break;
          }
        }
      }
      _ => {}
    }

    // 转换为数字
    let number = if base == 16 {
      // 跳过前缀 0x
      match u64::from_str_radix(string.trim_start_matches("0x").trim_start_matches("0X"), 16) {
        Ok(n) => n as f64 * sign,
        Err(_) => return Err(self.error("Bad hex number".to_string())),
      }
    } else {
      match string.parse::<f64>() {
        Ok(n) => n * sign,
        Err(_) => return Err(self.error("Bad number".to_string())),
      }
    };

    if !number.is_finite() {
      return Err(self.error("Bad number".to_string()));
    }

    // 判断是否可以安全转为整数
    if is_float {
      if number.fract() == 0.0 && number >= (i64::MIN as f64) && number <= (i64::MAX as f64) {
        // 可以安全转为整数
        let int_val = number as i128;
        match serde_json::Number::from_i128(int_val) {
          Some(num) => Ok(Value::Number(num)),
          None => Err(self.error("Bad number".to_string())),
        }
      } else {
        // 只能用浮点数
        match serde_json::Number::from_f64(number) {
          Some(num) => Ok(Value::Number(num)),
          None => Err(self.error("Bad number".to_string())),
        }
      }
    } else {
      // 原本就是整数
      if number >= (i64::MIN as f64) && number <= (i64::MAX as f64) {
        let int_val = number as i128;
        match serde_json::Number::from_i128(int_val) {
          Some(num) => Ok(Value::Number(num)),
          None => Err(self.error("Bad number".to_string())),
        }
      } else if number >= 0.0 && number <= (u64::MAX as f64) {
        let uint_val = number as u128;
        match serde_json::Number::from_u128(uint_val) {
          Some(num) => Ok(Value::Number(num)),
          None => Err(self.error("Bad number".to_string())),
        }
      } else {
        // 超大整数只能用浮点数
        match serde_json::Number::from_f64(number) {
          Some(num) => Ok(Value::Number(num)),
          None => Err(self.error("Bad number".to_string())),
        }
      }
    }
  }

  /// Parse a string value.
  fn string(&mut self) -> ParseResult<Value> {
    // 检查起始引号
    let delim = match self.ch {
      Some('"') | Some('\'') => self.ch.unwrap(),
      _ => return Err(self.error("Bad string: expected starting quote".to_string())),
    };
    let mut result = String::new();

    // 进入字符串内容
    while let Some(_) = self.next(None)? {
      if self.ch == Some(delim) {
        self.next(None)?; // 跳过结束引号
        return Ok(Value::String(result));
      } else if self.ch == Some('\\') {
        self.next(None)?;
        match self.ch {
          Some('u') => {
            // 处理 \uXXXX
            let mut uffff = 0u32;
            for _ in 0..4 {
              self.next(None)?;
              let hex = self.ch.and_then(|c| c.to_digit(16));
              if let Some(h) = hex {
                uffff = uffff * 16 + h;
              } else {
                return Err(self.error("Invalid Unicode escape in string".to_string()));
              }
            }
            if let Some(ch) = std::char::from_u32(uffff) {
              result.push(ch);
            } else {
              return Err(self.error("Invalid Unicode codepoint in string".to_string()));
            }
          }
          Some('\r') => {
            // 处理 \r\n 换行
            if self.peek() == Some('\n') {
              self.next(None)?;
            }
          }
          Some(esc) => {
            if let Some(mapped) = escapee_get(esc) {
              result.push_str(mapped);
            } else {
              // 非法转义，直接报错
              return Err(self.error(format!("Invalid escape character: {}", esc)));
            }
          }
          None => return Err(self.error("Unexpected end of input in string escape".to_string())),
        }
      } else if self.ch == Some('\r') {
        // 跳过裸 \r
      } else if self.ch == Some('\n') {
        // 允许裸 \n，直接加入
        result.push('\n');
      } else if let Some(ch) = self.ch {
        result.push(ch);
      } else {
        break;
      }
    }
    Err(self.error("Bad string".to_string()))
  }

  // 跳过单行注释
  fn inline_comment(&mut self) -> ParseResult<()> {
    if self.ch != Some('/') {
      return Err(self.error("Not an inline comment".to_string()));
    }
    loop {
      self.next(None)?;
      match self.ch {
        Some('\n') | Some('\r') => {
          self.next(None)?; // 跳过换行符
          return Ok(());
        }
        None => return Ok(()), // 文件结尾也算注释结束
        _ => {}
      }
    }
  }

  // 跳过多行注释
  fn block_comment(&mut self) -> ParseResult<()> {
    if self.ch != Some('*') {
      return Err(self.error("Not a block comment".to_string()));
    }
    loop {
      self.next(None)?;
      while self.ch == Some('*') {
        self.next(Some('*'))?;
        if self.ch == Some('/') {
          self.next(Some('/'))?;
          return Ok(());
        }
      }
      if self.ch.is_none() {
        return Err(self.error("Unterminated block comment".to_string()));
      }
    }
  }

  // 跳过注释（自动判断类型）
  fn comment(&mut self) -> ParseResult<()> {
    if self.ch != Some('/') {
      return Err(self.error("Not a comment".to_string()));
    }
    self.next(Some('/'))?;
    match self.ch {
      Some('/') => self.inline_comment(),
      Some('*') => self.block_comment(),
      _ => Err(self.error("Unrecognized comment".to_string())),
    }
  }

  // 跳过空白和注释
  fn white(&mut self) -> ParseResult<()> {
    loop {
      match self.ch {
        Some('/') => {
          self.comment()?;
        }
        Some(c) if WS.contains(&c) => {
          // 跳过空白字符
          let _ = self.next(None);
        }
        _ => {
          // 非空白、非注释，退出
          return Ok(());
        }
      }
    }
  }

  fn word(&mut self) -> ParseResult<Value> {
    match self.ch {
      Some('t') => {
        self.next(Some('t'))?;
        self.next(Some('r'))?;
        self.next(Some('u'))?;
        self.next(Some('e'))?;
        Ok(Value::Bool(true))
      }
      Some('f') => {
        self.next(Some('f'))?;
        self.next(Some('a'))?;
        self.next(Some('l'))?;
        self.next(Some('s'))?;
        self.next(Some('e'))?;
        Ok(Value::Bool(false))
      }
      Some('n') => {
        self.next(Some('n'))?;
        self.next(Some('u'))?;
        self.next(Some('l'))?;
        self.next(Some('l'))?;
        Ok(Value::Null)
      }
      Some('I') => {
        self.next(Some('I'))?;
        self.next(Some('n'))?;
        self.next(Some('f'))?;
        self.next(Some('i'))?;
        self.next(Some('n'))?;
        self.next(Some('i'))?;
        self.next(Some('t'))?;
        self.next(Some('y'))?;
        Ok(Value::String("Infinity".to_string()))
      }
      Some('N') => {
        self.next(Some('N'))?;
        self.next(Some('a'))?;
        self.next(Some('N'))?;
        Ok(Value::String("NaN".to_string()))
      }
      _ => Err(self.error(format!(
        "Unexpected {}",
        self.ch.map_or("EOF".to_string(), |x| render_char(x))
      ))),
    }
  }

  fn array(&mut self) -> ParseResult<Value> {
    let mut arr = Vec::new();
    let mut had_comma = false;

    // 必须以 [ 开头
    if self.ch == Some('[') {
      self.next(Some('['))?;
      self.white()?;
      loop {
        match self.ch {
          Some(']') => {
            if had_comma {
              return Err(self.error("Superfluous trailing comma".to_string()));
            }
            self.next(Some(']'))?;
            return Ok(Value::Array(arr));
          }
          Some(',') => {
            return Err(self.error("Missing array element".to_string()));
          }
          Some(_) => {
            arr.push(self.value()?);
          }
          None => break,
        }
        self.white()?;
        if self.ch != Some(',') {
          self.next(Some(']'))?;
          return Ok(Value::Array(arr));
        }
        self.next(Some(','))?;
        had_comma = true;
        self.white()?;
      }
    }
    Err(self.error("Bad array".to_string()))
  }

  fn object(&mut self) -> ParseResult<Value> {
    let mut obj = serde_json::Map::new();
    let mut had_comma = false;

    if self.ch == Some('{') {
      self.next(Some('{'))?;
      self.white()?;
      loop {
        match self.ch {
          Some('}') => {
            if had_comma {
              return Err(self.error("Superfluous trailing comma".to_string()));
            }
            self.next(Some('}'))?;
            return Ok(Value::Object(obj));
          }
          Some('"') | Some('\'') => {
            // 只允许带引号的key
            let key_val = self.string()?;
            let key = if let Value::String(s) = key_val {
              s
            } else {
              return Err(self.error("Object key must be a string".to_string()));
            };
            self.white()?;
            self.next(Some(':'))?;
            let value = self.value()?;
            obj.insert(key, value);
          }
          Some(',') => {
            return Err(self.error("Expected key".to_string()));
          }
          Some(_) => {
            // 不允许未加引号的key
            return Err(self.error("Unquoted key".to_string()));
          }
          None => break,
        }
        self.white()?;
        if self.ch != Some(',') {
          self.next(Some('}'))?;
          return Ok(Value::Object(obj));
        }
        self.next(Some(','))?;
        had_comma = true;
        self.white()?;
      }
    }
    Err(self.error("Bad object".to_string()))
  }

  fn value(&mut self) -> ParseResult<Value> {
    self.white()?;
    match self.ch {
      Some('{') => self.object(),
      Some('[') => self.array(),
      Some('"') | Some('\'') => self.string(),
      Some('-') | Some('+') | Some('.') => self.number(),
      Some(c) if c.is_digit(10) => self.number(),
      _ => self.word(),
    }
  }
}

// 转义字符映射
fn render_char(c: char) -> String {
  if c == '\0' {
    "EOF".to_string()
  } else {
    format!("'{}'", c)
  }
}

// 对外接口
pub fn parse(text: &str) -> ParseResult<Value> {
  let mut parser = Parser::new(text);
  let result = parser.value()?;
  parser.white()?;
  if parser.ch.is_some() {
    return Err(parser.error("Syntax error".to_string()));
  }
  Ok(result)
}
