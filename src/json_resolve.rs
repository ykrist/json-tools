use clap::{Args, Parser};
use json_tools::*;
use posix_cli_utils::*;
use regex::Regex;
use serde::{Serialize, Serializer};
use serde_json::Value;
use std::path::PathBuf;

#[derive(Debug, Clone, Args)]
struct Resolve {
    /// Print error messages to STDERR when files match the regex but cannot be opened
    #[clap(short = 'v')]
    verbose: bool,
    /// Set the regex used to identify strings as filenames
    #[clap(short='m', parse(try_from_str=Regex::new), default_value=r"\.json$")]
    regex: Regex,
    /// Enable recursive resolution
    #[clap(short = 'r')]
    recursion: bool,
    /// Specify directories to search in. If input is a file, default search path
    /// is the file's parent directory.  Otherwise the search path is the current working directory.
    #[clap(short = 'd')]
    directories: Vec<PathBuf>,
}

#[derive(Debug, Clone, Parser)]
struct ClArgs {
    /// Input JSON file (defaults to STDIN)
    input: Option<PathBuf>,
    #[clap(flatten)]
    options: Resolve,
}

impl Resolve {
    fn resolve(&self, val: &mut Value) {
        let filename = match val {
            Value::Array(list) => {
                list.iter_mut().for_each(|v| self.resolve(v));
                return;
            }

            Value::Object(map) => {
                map.values_mut().for_each(|v| self.resolve(v));
                return;
            }

            Value::String(s) if self.regex.is_match(s) => &*s,

            _ => return,
        };

        let mut replacement = None;
        for d in &self.directories {
            let p = d.join(filename);
            match load_json(p) {
                Ok(v) => {
                    replacement = Some(v);
                    break;
                }
                Err(e) => {
                    if self.verbose {
                        eprintln!("{:?}\n", e);
                    }
                }
            }
        }
        if let Some(mut replacement) = replacement {
            if self.recursion {
                self.resolve(&mut replacement);
            }
            *val = replacement;
        }
    }
}

impl RunStreamJson for Resolve {
    fn process_one<S>(&mut self, mut value: Value, output: S) -> Result<()>
    where
        S: Serializer,
        S::Error: Send + Sync + 'static,
    {
        self.resolve(&mut value);
        value.serialize(output)?;
        Ok(())
    }
}

fn main() -> Result<()> {
    reset_sigpipe();
    let mut args = ClArgs::parse();

    let input = Input::default_stdin(args.input.as_ref())?;

    if args.options.directories.is_empty() {
        if let Some(ref filename) = args.input {
            args.options
                .directories
                .push(filename.parent().unwrap().to_path_buf());
        } else {
            args.options.directories.push(std::env::current_dir()?);
        }
    }

    args.options.main(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn options() -> Resolve {
        Resolve {
            verbose: true,
            regex: Regex::new(r"\.json$").unwrap(),
            recursion: false,
            directories: vec!["tests/".into()],
        }
    }

    fn fake_run(input: impl AsRef<Path>, options: &Resolve) -> Result<Value> {
        let mut value = load_json(input)?;
        options.resolve(&mut value);
        Ok(value)
    }

    #[test]
    fn recursive() -> Result<()> {
        let mut o = options();
        o.recursion = true;
        let correct = load_json("tests/recursive.json")?;
        let x = fake_run("tests/root.json", &o)?;
        assert_eq!(x, correct);
        Ok(())
    }

    #[test]
    fn nonrecursive() -> Result<()> {
        let o = options();
        let correct = load_json("tests/nonrecursive.json")?;
        let x = fake_run("tests/root.json", &o)?;
        assert_eq!(x, correct);
        Ok(())
    }

    #[test]
    fn custom_pattern() -> Result<()> {
        let mut o = options();
        o.regex = Regex::new(r"d\.json$")?;
        let correct = load_json("tests/donly.json")?;
        let x = fake_run("tests/root.json", &o)?;
        assert_eq!(x, correct);
        Ok(())
    }

    #[test]
    fn wrong_directory() -> Result<()> {
        let mut o = options();
        o.directories[0] = "./".into();
        let correct = load_json("tests/root.json")?;
        let x = fake_run("tests/root.json", &o)?;
        assert_eq!(x, correct);
        Ok(())
    }
}
