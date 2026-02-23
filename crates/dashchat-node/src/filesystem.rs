use std::path::PathBuf;

/// Wrapper type to centralize all filesystem path management for Dash Chat
///
/// The Node struct receives a data path for the folder that it manages,
/// and uses this struct to name each of the files

#[derive(Clone)]
pub struct Filesystem(PathBuf);

impl Filesystem {
    pub fn new(data_path: PathBuf) -> Self {
        Self(data_path)
    }

    pub fn local_store_path(&self) -> PathBuf {
        self.0.join("localdata.redb")
    }

    pub fn op_store_path(&self) -> PathBuf {
        self.0.join("opstore.db")
    }
}
