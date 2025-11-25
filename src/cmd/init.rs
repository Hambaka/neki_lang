use std::fs;

use anyhow::{Context, Result, bail};

use crate::cmd::shared::{DEFAULT_DIR_CONFIG, DEFAULT_REGEX_CONFIG};

enum ConfigStatus {
  AllExist,
  DirExists,
  RegexExists,
  NoneExists,
}

pub fn run(force: bool) -> Result<()> {
  println!("[INFO] Initializing configuration files...");

  let exe_dir = std::env::current_exe()
    .context("[ERROR] Failed to get current executable path!")?
    .parent()
    .context("[ERROR] Failed to get parent directory of executable!")?
    .to_path_buf();

  let dir_path = exe_dir.join("dirs_config.json");
  let regex_path = exe_dir.join("regex_config.json");

  if !force {
    let status = match (dir_path.exists(), regex_path.exists()) {
      (true, true) => ConfigStatus::AllExist,
      (true, false) => ConfigStatus::DirExists,
      (false, true) => ConfigStatus::RegexExists,
      (false, false) => ConfigStatus::NoneExists,
    };

    match status {
      ConfigStatus::AllExist => {
        bail!(
          "[ERROR] All config files already exist in {}. Use --force to overwrite.",
          exe_dir.display()
        )
      }
      ConfigStatus::DirExists => {
        println!(
          "[WARN] \"dirs_config.json\" already exists in {}. Use --force to overwrite.",
          exe_dir.display()
        );
        println!("[INFO] Writing regex_config.json...");

        fs::write(&regex_path, DEFAULT_REGEX_CONFIG).context(format!(
          "[ERROR] Failed to write \"regex_config.json\" to {}",
          regex_path.display()
        ))?;
      }
      ConfigStatus::RegexExists => {
        println!(
          "[WARN] \"regex_config.json\" already exists in {}. Use --force to overwrite.",
          exe_dir.display()
        );
        println!("[INFO] Writing \"dirs_config.json\"...");

        fs::write(&dir_path, DEFAULT_DIR_CONFIG).context(format!(
          "[ERROR] Failed to write \"dirs_config.json\" to {}",
          dir_path.display()
        ))?;
      }
      ConfigStatus::NoneExists => {
        println!("[INFO] Writing \"dirs_config.json\" and \"regex_config.json\"...");
        fs::write(&dir_path, DEFAULT_DIR_CONFIG).context(format!(
          "[ERROR] Failed to write \"dirs_config.json\" to {}",
          dir_path.display()
        ))?;
        fs::write(&regex_path, DEFAULT_REGEX_CONFIG).context(format!(
          "[ERROR] Failed to write \"regex_config.json\" to {}",
          regex_path.display()
        ))?;
      }
    }
  }
  println!("[INFO] Writing \"dirs_config.json\" and \"regex_config.json\"...");
  fs::write(&dir_path, DEFAULT_DIR_CONFIG).context(format!(
    "[ERROR] Failed to write \"dirs_config.json\" to {}",
    dir_path.display()
  ))?;
  fs::write(&regex_path, DEFAULT_REGEX_CONFIG).context(format!(
    "[ERROR] Failed to write \"regex_config.json\" to {}",
    regex_path.display()
  ))?;

  println!(
    "[INFO] Configuration files initialized in {}",
    exe_dir.display()
  );

  Ok(())
}
