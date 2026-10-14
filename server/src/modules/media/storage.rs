use crate::AppError;
use std::{
    fs::{DirBuilder, File, OpenOptions},
    io::Write,
    path::PathBuf,
};
fn io(e: std::io::Error) -> AppError {
    AppError::Internal(format!("Private media storage: {e}"))
}
pub(super) fn write(root: PathBuf, target: PathBuf, data: Vec<u8>) -> Result<File, AppError> {
    let mut directory = DirBuilder::new();
    directory.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        directory.mode(0o700);
    }
    directory.create(root).map_err(io)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    // create_new must succeed before compensation may remove this path.
    let mut file = options.open(&target).map_err(io)?;
    let result = (|| {
        file.try_lock()
            .map_err(|e| AppError::Internal(format!("Media file lock: {e}")))?;
        file.write_all(&data).map_err(io)?;
        file.sync_all().map_err(io)?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = std::fs::remove_file(target);
        return Err(error);
    }
    Ok(file)
}
