use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{Diagnostic, NO_LOCATION};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

pub(crate) struct Workspace(PathBuf);

impl Workspace {
    pub(crate) fn new() -> Result<Self, Diagnostic> {
        loop {
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("xcx-legacy-aot-{}-{id}", std::process::id()));
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(Diagnostic::new(
                        format!("cannot create the compilation workspace: {error}"),
                        NO_LOCATION,
                    ));
                }
            }
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }

    pub(crate) fn executable(&self) -> PathBuf {
        self.0
            .join(format!("program{}", std::env::consts::EXE_SUFFIX))
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
