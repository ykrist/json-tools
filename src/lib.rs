use posix_cli_utils::*;
use serde::Serializer;
use serde_json::{de::IoRead, Deserializer, Value};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

pub trait RunStreamJson: Sized {
    fn process_one<S>(&mut self, value: Value, output: S) -> Result<()>
    where
        S: Serializer,
        S::Error: Send + Sync + 'static;

    fn main<R: Read>(&mut self, input: Input<R>) -> Result<()> {
        match input {
            Input::File(file) => run_json_stream_impl(file, self),
            Input::Stdin(input) => run_json_stream_impl(input, self),
        }
    }
}

fn run_json_stream_impl<R, T>(input: R, run: &mut T) -> Result<()>
where
    T: RunStreamJson,
    R: Read,
{
    let stream = Deserializer::new(IoRead::new(input)).into_iter::<Value>();
    let mut stdout = std::io::stdout();

    for value in stream {
        let mut output = serde_json::Serializer::new(stdout.lock());
        run.process_one(value?, &mut output)?;
        drop(output);
        stdout.write_all(b"\n")?;
    }
    Ok(())
}

pub fn load_json(path: impl AsRef<Path>) -> Result<Value> {
    let path = path.as_ref();
    let file = File::open(path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_reader(file).with_context(|| format!("failed to parse {}", path.display()))
}

pub trait ValueExt {
    fn kind(&self) -> &'static str;
}

impl ValueExt for Value {
    fn kind(&self) -> &'static str {
        use Value::*;
        match self {
            Array(_) => "array",
            Object(_) => "object",
            Null => "null",
            String(_) => "string",
            Number(_) => "number",
            Bool(_) => "boolean",
        }
    }
}
