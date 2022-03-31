pub use anyhow::{Result, anyhow, bail, Context};
use std::fs::File;
use std::io::{self, BufReader};
use std::path::Path;

#[cfg(unix)]
pub fn reset_sigpipe() {
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

#[cfg(not(unix))]
pub fn reset_sigpipe() {
    // no-op
}

pub enum Input {
    File(BufReader<File>),
    Stdin(io::Stdin),
}

pub fn open_file_or_stdin<P: AsRef<Path>>(path: Option<P>) -> Result<Input> {
    if let Some(path) = path {
        let path = path.as_ref();
        File::open(path)
            .map(BufReader::new)
            .map(Input::File)
            .with_context(|| format!("unable to read {}", path.display()))
    } else {
        Ok(Input::Stdin(std::io::stdin()))
    }
}
