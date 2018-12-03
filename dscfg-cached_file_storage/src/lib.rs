extern crate dscfg_server;
extern crate serde_json;
extern crate void;

use dscfg_server::{IsFatalError, Storage};
use std::path::{Path, PathBuf};
use std::io;
use std::fs::File;
use std::collections::HashMap;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum IoOperation {
    Open(PathBuf),
    Write(PathBuf),
    Move(PathBuf, PathBuf),
}

#[derive(Debug)]
pub struct StorageError {
    operation: IoOperation,
    error: io::Error,
}

impl StorageError {
    fn open_error(file: impl Into<PathBuf>, error: io::Error) -> Self {
        StorageError {
            operation: IoOperation::Open(file.into()),
            error,
        }
    }

    fn write_error(file: impl Into<PathBuf>, error: impl Into<io::Error>) -> Self {
        StorageError {
            operation: IoOperation::Write(file.into()),
            error: error.into(),
        }
    }

    fn move_error(from: impl Into<PathBuf>, to: impl Into<PathBuf>, error: io::Error) -> Self {
        StorageError {
            operation: IoOperation::Move(from.into(), to.into()),
            error,
        }
    }
}

impl IsFatalError for StorageError {
    fn is_fatal(&self) -> bool {
        if let IoOperation::Write(_) = self.operation {
            true
        } else {
            self.error.kind() != io::ErrorKind::Interrupted && 
            self.error.kind() != io::ErrorKind::WouldBlock
        }
    }
}

pub struct CachedFileStorage {
    file_path: PathBuf,
    temp_file: PathBuf,
    data: HashMap<String, serde_json::Value>,
}

impl CachedFileStorage {
    pub fn load_or_create<P: AsRef<Path> + Into<PathBuf>>(file: P) -> io::Result<Self> {
        let data = match File::open(file.as_ref()) {
            Ok(file) => serde_json::from_reader(file)?,
            Err(ref err) if err.kind() == io::ErrorKind::NotFound => Default::default(),
            Err(err) => return Err(err),
        };

        let temp_file = Self::temp_file_path(file.as_ref())?;
        let file_path = file.into();

        Ok(CachedFileStorage {
            file_path,
            temp_file,
            data,
        })
    }

    fn temp_file_path(original_path: &Path) -> io::Result<PathBuf> {
        use std::ffi::OsString;

        let file_name = original_path.file_name().ok_or(io::ErrorKind::InvalidInput)?;
        let mut temp_file_name: OsString = ".".into();
        temp_file_name.push(file_name);
        temp_file_name.push(".tmp");
        let mut temp_file = original_path.parent().map(PathBuf::from).unwrap_or_else(PathBuf::new);
        temp_file.push(temp_file_name);

        Ok(temp_file)
    }
}

impl Storage for CachedFileStorage {
    type SetError = StorageError;
    type GetError = void::Void;

    fn set(&mut self, key: String, value: serde_json::Value) -> Result<(), Self::SetError> {
        // TODO: restore original state on failure
        self.data.insert(key, value);
        // Make sure the file is closed before renaming.
        {
            let mut file = File::create(&self.temp_file).map_err(|err| StorageError::open_error(&self.temp_file, err))?;
            serde_json::to_writer(&mut file, &self.data).map_err(|err| StorageError::write_error(&self.temp_file, err))?;
            file.sync_data().map_err(|err| StorageError::write_error(&self.temp_file, err))?;
        }
        std::fs::rename(&self.temp_file, &self.file_path).map_err(|err| StorageError::move_error(&self.temp_file, &self.file_path, err))?;
        Ok(())
    }

    fn get(&mut self, key: &str) -> Result<Option<serde_json::Value>, Self::GetError> {
        Ok(self.data.get(key).map(Clone::clone))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
