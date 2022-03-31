use json_tools::*;
use regex::Regex;
use std::{path::{PathBuf, Path}, io::{Write, Read, stdin, stdout}, fs::File};
use serde_json::{Value, Deserializer, de::IoRead};
use clap::{Args, Parser};

#[derive(Debug, Clone, Args)]
struct Options {
    /// Print error messages to STDERR when files match the regex but cannot be opened
    #[clap(short='v')]
    verbose: bool,
    /// Set the regex used to identify strings as filenames
    #[clap(short='m', parse(try_from_str=Regex::new), default_value=r"\.json$")]
    regex: Regex,
    /// Enable recursive resolution
    #[clap(short='r')]
    recursion: bool,
    /// Specify directories to search in. If input is a file, default search path
    /// is the file's parent directory.  Otherwise the search path is the current working directory.
    #[clap(short='d')]
    directories: Vec<PathBuf>,
}

#[derive(Debug, Clone, Parser)]
struct ClArgs {
    /// Input JSON file (defaults to STDIN)
    input: Option<PathBuf>,
    #[clap(flatten)]
    options: Options
}

fn load_json(path: impl AsRef<Path>) -> Result<Value> {
    let path = path.as_ref();
    let file = File::open(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_reader(file)
        .with_context(|| format!("failed to parse {}", path.display()))
}

fn resolve(val: &mut Value, options: &Options) {
    let filename = match val {
        Value::Array(list) => {
            list.iter_mut().for_each(|v| resolve(v, options));
            return;
        }
            
        Value::Object(map) => {
            map.values_mut().for_each(|v| resolve(v, options));
            return;
        }
            
        Value::String(s) if options.regex.is_match(s) => &*s,

        _ => return,
    };

    let mut replacement = None;
    for d in &options.directories {
        let p = d.join(filename);
        match load_json(p) {
            Ok(v) => { 
                replacement = Some(v);
                break;
            },
            Err(e) => {
                if options.verbose {
                    eprintln!("{:?}", e);
                }
            }
        }
    }
    if let Some(mut replacement) = replacement {
        if options.recursion {
            resolve(&mut replacement, &options);
        }
        *val = replacement;
    }
}

fn run(input: impl Read, mut output: impl Write, options: &Options) -> Result<()> {
    let stream = Deserializer::new(IoRead::new(input)).into_iter::<Value>();
    for value in stream {
        let mut value = value?;
        resolve(&mut value, options);
        serde_json::to_writer(&mut output, &value)?;
        output.write_all(b"\n")?;
    }
    Ok(())
}

fn main() -> Result<()> {
    reset_sigpipe();
    let mut args = ClArgs::parse();
    let output = stdout();
    let output_lock = output.lock();

    if let Some(filename) = args.input.as_ref() {
        let file = File::open(filename)
        .   with_context(|| format!("failed to open {}", filename.display()))?;
        if args.options.directories.is_empty() {
            if let Some(dir) = filename.parent() {
                args.options.directories.push(dir.to_path_buf());
            } else {
                args.options.directories.push(std::env::current_dir()?);
            }
        }
        run(file, output_lock, &args.options)?;
    } else {
        if args.options.directories.is_empty() {
            args.options.directories.push(std::env::current_dir()?);
        }
        let input = stdin();
        run(input.lock(), output_lock, &args.options)?;
    }

    Ok(())
}