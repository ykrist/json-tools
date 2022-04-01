use std::{
    fmt::{Display, Write as FmtWrite},
    io::{self, Read, StdoutLock, Write},
    path::PathBuf,
};

use indexmap::IndexMap;
use json_tools::ValueExt;
use posix_cli_utils::*;
use serde_json::{de::IoRead, Value};

v_escape::new!(EscapeQuotes; '"' -> r#"\""#);

#[derive(Debug, Clone, Parser)]
struct ClArgs {
    /// Input JSON file (defaults to STDIN)
    input: Option<PathBuf>,
    #[clap(flatten)]
    options: Json2Csv,
}

/// Convert a stream of JSON object records to CSV, one object per row.
#[derive(Clone, Debug, Args)]
struct Json2Csv {
    /// Set the output CSV delimiter
    #[clap(short = 'd', default_value = ",")]
    delimiter: String,
    /// Put strings in double quotes, escaping double quotes with backslashes.
    /// For example `this, string " has, commas and quotes` becomes `"this, string \" has, commas and quotes"`
    #[clap(short = 'q')]
    quote_strings: bool,
}

fn write_delimited<W, I>(mut writer: W, values: I, delim: &str) -> Result<()>
where
    W: Write,
    I: IntoIterator,
    I::Item: Display,
{
    let mut values = values.into_iter();
    if let Some(v) = values.next() {
        write!(writer, "{}", v)?;
    }
    for v in values {
        write!(writer, "{}{}", delim, v)?;
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum OutputField {
    Empty,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    QuotedString(String),
}

impl Display for OutputField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use OutputField::*;
        match self {
            Empty => Ok(()),
            Bool(false) => f.write_char('0'),
            Bool(true) => f.write_char('1'),
            Number(n) => Display::fmt(n, f),
            String(s) => Display::fmt(s, f),
            QuotedString(s) => {
                f.write_char('"')?;
                Display::fmt(&escape(s), f)?;
                f.write_char('"')?;
                Ok(())
            }
        }
    }
}

impl Json2Csv {
    fn run(&self, input: impl Read, mut output: StdoutLock) -> Result<()> {
        let mut header = IndexMap::new();
        let mut rows = Vec::new();

        for value in serde_json::Deserializer::new(IoRead::new(input)).into_iter::<Value>() {
            let object = match value? {
                Value::Object(m) => m,
                other => bail!("expected JSON object, not {}", other.kind()),
            };
            let mut row = vec![OutputField::Empty; header.len()];
            for (key, value) in object {
                let value = match value {
                    Value::Array(_) | Value::Object(_) => continue,
                    Value::String(s) => {
                        if self.quote_strings {
                            OutputField::QuotedString(s)
                        } else {
                            OutputField::String(s)
                        }
                    }
                    Value::Bool(b) => OutputField::Bool(b),
                    Value::Number(n) => OutputField::Number(n),
                    Value::Null => OutputField::Empty,
                };

                if let Some(idx) = header.get(&key).copied() {
                    row[idx] = value;
                } else {
                    header.insert(key, header.len());
                    row.push(value);
                    debug_assert_eq!(header.len() - 1, row.len() - 1);
                }
            }
            rows.push(row);
        }

        let ncols = header.len();
        if self.quote_strings {
            write_delimited(
                &mut output,
                header.into_keys().map(OutputField::QuotedString),
                &self.delimiter,
            )?;
        } else {
            write_delimited(&mut output, header.keys(), &self.delimiter)?;
        }

        writeln!(&mut output)?;
        for row in &rows {
            let tail = std::iter::repeat(&OutputField::Empty).take(ncols - row.len());
            write_delimited(&mut output, row.iter().chain(tail), &self.delimiter)?;
            writeln!(&mut output)?;
        }

        Ok(())
    }
}

fn main() -> Result<()> {
    let ClArgs {
        input,
        options: json2csv,
    } = ClArgs::parse();
    let stdout = io::stdout();
    let output = stdout.lock();

    match Input::default_stdin(input)? {
        Input::File(f) => json2csv.run(f, output),
        Input::Stdin(i) => json2csv.run(i.lock(), output),
    }
}
