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
    fn type_name(&self) -> &'static str;
    fn unwrap_array(self) -> Vec<Value>;
    fn unwrap_object(self) -> serde_json::Map<String, Value>;
    fn unwrap_string(self) -> String;
    fn unwrap_str(&self) -> &str;

    fn expect_string(self) -> Result<String>;
    fn expect_object(self) -> Result<serde_json::Map<String, Value>>;
    fn expect_array(self) -> Result<Vec<Value>>;
    fn expect_number(self) -> Result<serde_json::Number>;
    fn expect_int(self) -> Result<i64>;
    fn expect_uint(self) -> Result<u64>;
}

impl ValueExt for Value {
    fn type_name(&self) -> &'static str {
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

    #[track_caller]
    #[inline]
    fn unwrap_array(self) -> Vec<Value> {
        match self {
            Value::Array(arr) => arr,
            other => panic!("expected JSON array: {}", other),
        }
    }

    #[track_caller]
    #[inline]
    fn unwrap_object(self) -> serde_json::Map<String, Value> {
        match self {
            Value::Object(v) => v,
            other => panic!("expected JSON object: {}", other),
        }
    }

    #[track_caller]
    #[inline]
    fn unwrap_string(self) -> String {
        match self {
            Value::String(v) => v,
            other => panic!("expected string: {}", other),
        }
    }

    #[track_caller]
    #[inline]
    fn unwrap_str(&self) -> &str {
        match self {
            Value::String(v) => v.as_str(),
            other => panic!("expected string: {}", other),
        }
    }

    fn expect_string(self) -> Result<String> {
        match self {
            Value::String(s) => Ok(s),
            other => bail!("expected JSON string, not {}", other.type_name()),
        }
    }

    fn expect_object(self) -> Result<serde_json::Map<String, Value>> {
        match self {
            Value::Object(v) => Ok(v),
            other => bail!("expected JSON object, not {}", other.type_name()),
        }
    }

    fn expect_array(self) -> Result<Vec<Value>> {
        match self {
            Value::Array(v) => Ok(v),
            other => bail!("expected JSON array, not {}", other.type_name()),
        }
    }

    fn expect_number(self) -> Result<serde_json::Number> {
        match self {
            Value::Number(v) => Ok(v),
            other => bail!("expected JSON number, not {}", other.type_name()),
        }
    }

    fn expect_int(self) -> Result<i64> {
        let n = self.expect_number()?;
        n.as_i64()
            .ok_or_else(|| anyhow!("cannot convert to integer: {}", n))
    }

    fn expect_uint(self) -> Result<u64> {
        let n = self.expect_number()?;
        n.as_u64()
            .ok_or_else(|| anyhow!("cannot convert to unsigned integer: {}", n))
    }
}
