use crate::utils::read_single_key;
use anyhow::Result;
use std::fs::OpenOptions;
use std::io::{self, BufRead, BufReader, Write};

pub fn confirm_high_risk(message: &str) -> Result<bool> {
    if let Ok(tty) = OpenOptions::new().read(true).write(true).open("/dev/tty") {
        let mut reader = BufReader::new(tty.try_clone()?);
        let mut writer = tty;
        write!(writer, "{message} [y/N] ")?;
        writer.flush()?;
        let mut answer = String::new();
        reader.read_line(&mut answer)?;
        return Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "YES"));
    }
    eprint!("{message} [y/N] ");
    io::stderr().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "YES"))
}

pub fn read_action(keys: &[char], default: char, prompt: &str) -> Result<char> {
    read_single_key(keys, default, prompt)
}
