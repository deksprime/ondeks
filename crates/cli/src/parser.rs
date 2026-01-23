//! Command parser for the CLI.

use std::collections::HashMap;

/// A parsed command with arguments.
#[derive(Debug, Clone)]
pub struct ParsedCommand {
    pub name: String,
    pub args: Vec<String>,
    pub flags: HashMap<String, Option<String>>,
}

impl ParsedCommand {
    /// Get positional argument by index.
    pub fn arg(&self, index: usize) -> Option<&str> {
        self.args.get(index).map(|s| s.as_str())
    }

    /// Get positional argument, returning error if missing.
    pub fn require_arg(&self, index: usize, name: &str) -> Result<&str, String> {
        self.arg(index)
            .ok_or_else(|| format!("Missing required argument: <{}>", name))
    }

    /// Parse positional argument as a type.
    pub fn parse_arg<T: std::str::FromStr>(&self, index: usize, name: &str) -> Result<T, String> {
        let s = self.require_arg(index, name)?;
        s.parse()
            .map_err(|_| format!("Invalid {}: '{}'", name, s))
    }

    /// Get optional flag value.
    pub fn flag(&self, name: &str) -> Option<&str> {
        self.flags.get(name).and_then(|v| v.as_deref())
    }

    /// Check if flag is present.
    pub fn has_flag(&self, name: &str) -> bool {
        self.flags.contains_key(name)
    }
}

/// Parse a command line into structured form.
pub fn parse_command(input: &str) -> Option<ParsedCommand> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }

    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut escape_next = false;

    for ch in input.chars() {
        if escape_next {
            current.push(ch);
            escape_next = false;
            continue;
        }

        match ch {
            '\\' => escape_next = true,
            '"' => in_quotes = !in_quotes,
            ' ' if !in_quotes => {
                if !current.is_empty() {
                    parts.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }

    if parts.is_empty() {
        return None;
    }

    let name = parts.remove(0);
    let mut args = Vec::new();
    let mut flags = HashMap::new();

    let mut i = 0;
    while i < parts.len() {
        let part = &parts[i];
        if part.starts_with("--") {
            let flag_name = part.trim_start_matches("--");
            if let Some(eq_pos) = flag_name.find('=') {
                let (name, value) = flag_name.split_at(eq_pos);
                flags.insert(name.to_string(), Some(value[1..].to_string()));
            } else if i + 1 < parts.len() && !parts[i + 1].starts_with("--") {
                flags.insert(flag_name.to_string(), Some(parts[i + 1].clone()));
                i += 1;
            } else {
                flags.insert(flag_name.to_string(), None);
            }
        } else if part.starts_with('-') {
            let flag_name = part.trim_start_matches('-');
            flags.insert(flag_name.to_string(), None);
        } else {
            args.push(part.clone());
        }
        i += 1;
    }

    Some(ParsedCommand { name, args, flags })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_command() {
        let cmd = parse_command("play").unwrap();
        assert_eq!(cmd.name, "play");
        assert!(cmd.args.is_empty());
    }

    #[test]
    fn parse_with_args() {
        let cmd = parse_command("tempo 140").unwrap();
        assert_eq!(cmd.name, "tempo");
        assert_eq!(cmd.arg(0), Some("140"));
    }

    #[test]
    fn parse_with_flags() {
        let cmd = parse_command("export output.wav --normalize --format wav").unwrap();
        assert_eq!(cmd.name, "export");
        assert_eq!(cmd.arg(0), Some("output.wav"));
        assert!(cmd.has_flag("normalize"));
        assert_eq!(cmd.flag("format"), Some("wav"));
    }

    #[test]
    fn parse_quoted_args() {
        let cmd = parse_command("add-track midi \"My Bass Track\"").unwrap();
        assert_eq!(cmd.arg(1), Some("My Bass Track"));
    }
}
