use anyhow::{Context, Result};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

/// Replace a file without exposing partial data. Caller owns the destination.
pub(crate) fn atomic_write(path: &Path, contents: &[u8], mode: Option<u32>) -> Result<()> {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let temp = path.with_extension(format!("tmp-{}-{nonce}", std::process::id()));
    let result = (|| {
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        if let Some(mode) = mode {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(mode);
        }
        #[cfg(not(unix))]
        let _ = mode;
        let mut file = options
            .open(&temp)
            .with_context(|| format!("failed to write {}", temp.display()))?;
        file.write_all(contents)?;
        file.sync_all()?;
        fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}
