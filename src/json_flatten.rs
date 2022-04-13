use std::{collections::HashMap, fmt::Display, path::PathBuf};

use indexmap::IndexMap;
use json_tools::*;
use posix_cli_utils::*;
use serde::Serialize;
use serde_json::Value;
use std::fmt::Write;

#[derive(Debug, Clone, Args)]
struct Flatten {
    /// Separater to use when concatenating keys
    #[clap(short = 'd', default_value = ".")]
    sep: String,
}

/// Recursively flatten a JSON object.
#[derive(Debug, Clone, Parser)]
struct Args {
    /// Input JSON file (defaults to STDIN)
    input: Option<PathBuf>,
    /// Unflatten instead
    #[clap(short = 'u')]
    unflatten: bool,
    #[clap(flatten)]
    options: Flatten,
}

#[derive(Serialize, Clone, Debug)]
#[serde(untagged)]
enum UnflattenTree {
    Branch(HashMap<String, UnflattenTree>),
    Empty,
    Leaf(Value),
}

impl UnflattenTree {
    fn has_children(&self) -> bool {
        matches!(self, UnflattenTree::Branch(_))
    }

    fn insert<'a>(&mut self, mut keys: impl Iterator<Item = &'a str>, value: Value) {
        if let Some(key) = keys.next() {
            match self {
                UnflattenTree::Empty | UnflattenTree::Leaf(_) => {
                    *self = UnflattenTree::Branch({
                        let mut m = HashMap::new();
                        m.entry(key.to_string())
                            .or_insert(UnflattenTree::Empty)
                            .insert(keys, value);
                        m
                    });
                }
                UnflattenTree::Branch(map) => {
                    if !map.contains_key(key) {
                        map.insert(key.to_string(), UnflattenTree::Empty);
                    }
                    map.get_mut(key).unwrap().insert(keys, value);
                }
            }
        } else if !self.has_children() {
            *self = UnflattenTree::Leaf(value);
        }
    }
}

impl Flatten {
    fn recurse<I, K>(
        self: &Flatten,
        output: &mut IndexMap<String, Value>,
        current_key: String,
        items: I,
    ) where
        K: Display,
        I: IntoIterator<Item = (K, Value)>,
    {
        for (k, val) in items {
            let mut key = current_key.clone();
            if key.len() == 0 {
                write!(key, "{}", k).unwrap();
            } else {
                write!(key, "{}{}", &self.sep, k).unwrap();
            }
            self.flatten(output, key, val);
        }
    }

    fn flatten(
        &self,
        output: &mut IndexMap<String, Value>,
        current_key: String,
        current_value: Value,
    ) {
        match current_value {
            Value::Array(items) => self.recurse(output, current_key, items.into_iter().enumerate()),
            Value::Object(items) => self.recurse(output, current_key, items),

            scalar => {
                output.insert(current_key, scalar);
            }
        }
    }

    fn unflatten(&self, input: Value) -> Result<UnflattenTree> {
        let input = match input {
            Value::Object(x) => x,
            _ => bail!("top-level object must be to be object type"),
        };
        let mut tree = UnflattenTree::Empty;

        for (key, value) in input {
            tree.insert(key.split(&*self.sep), value);
        }

        Ok(tree)
    }
}

impl RunStreamJson for Flatten {
    fn process_one<S>(&mut self, value: Value, output: S) -> Result<()>
    where
        S: serde::Serializer,
        S::Error: Send + Sync + 'static,
    {
        if value.is_object() || value.is_array() {
            let mut flat = IndexMap::new();
            self.flatten(&mut flat, String::new(), value);
            flat.serialize(output)?;
        } else {
            value.serialize(output)?;
        }
        Ok(())
    }
}

struct Unflatten(Flatten);

impl RunStreamJson for Unflatten {
    fn process_one<S>(&mut self, value: Value, output: S) -> Result<()>
    where
        S: serde::Serializer,
        S::Error: Send + Sync + 'static,
    {
        let value = self.0.unflatten(value)?;
        value.serialize(output)?;
        Ok(())
    }
}

fn main() -> Result<()> {
    reset_sigpipe();
    let mut args = Args::parse();
    let input = Input::default_stdin(args.input.as_ref())?;
    if args.unflatten {
        Unflatten(args.options).main(input)
    } else {
        args.options.main(input)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn options() -> Flatten {
        Flatten {
            sep: ".".to_string(),
        }
    }

    fn unflatten(value: Value) -> Value {
        let u = options().unflatten(value).unwrap();
        let u = serde_json::to_string(&u).unwrap();
        serde_json::from_str(&u).unwrap()
    }

    fn flatten(value: Value) -> Value {
        let mut m = IndexMap::new();
        options().flatten(&mut m, String::new(), value);
        let out = serde_json::to_string(&m).unwrap();
        serde_json::from_str(&out).unwrap()
    }

    #[test]
    fn check_flatten() -> Result<()> {
        let correct = load_json("tests/recursive-flat.json")?;
        let x = load_json("tests/recursive.json").map(flatten)?;
        assert_eq!(x, correct);
        Ok(())
    }

    #[test]
    fn check_unflatten() -> Result<()> {
        let correct = load_json("tests/recursive-flat-unflatten.json")?;
        let x = load_json("tests/recursive-flat.json").map(unflatten)?;
        assert_eq!(x, correct);
        Ok(())
    }

    #[test]
    fn clobber() {
        let original = json! ({
            "a.b" : [1u8],
            "a" : 2u8,
        });
        let unflat = json!({
            "a" : { "b" : [1u8] },
        });
        assert_eq!(unflatten(original), unflat);
    }

    #[test]
    #[should_panic]
    fn bad_top_level_object() {
        unflatten(Value::Null);
    }

    #[test]
    fn simple() {
        let original = json! ({
            "a" : { "b" : 1u8 },
        });
        let flat = json! ({
            "a.b" : 1u8,
        });
        assert_eq!(flatten(original), flat);
    }
}
