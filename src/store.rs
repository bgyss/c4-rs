use crate::id::{identify, Id};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Cursor, Read};
use std::path::{Path, PathBuf};

pub trait Store {
    fn has(&self, id: &Id) -> bool;
    fn get(&self, id: &Id) -> io::Result<Option<Vec<u8>>>;
    fn put<R: Read>(&mut self, reader: R) -> io::Result<Id>;
}

#[derive(Default)]
pub struct MemoryStore {
    objects: HashMap<Id, Vec<u8>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Store for MemoryStore {
    fn has(&self, id: &Id) -> bool {
        self.objects.contains_key(id)
    }

    fn get(&self, id: &Id) -> io::Result<Option<Vec<u8>>> {
        Ok(self.objects.get(id).cloned())
    }

    fn put<R: Read>(&mut self, mut reader: R) -> io::Result<Id> {
        let mut data = Vec::new();
        reader.read_to_end(&mut data)?;
        let id = identify(Cursor::new(&data))?;
        self.objects.entry(id).or_insert(data);
        Ok(id)
    }
}

pub struct FolderStore {
    root: PathBuf,
}

impl FolderStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn path_for(&self, id: &Id) -> PathBuf {
        let text = id.to_string();
        self.root.join(&text[0..4]).join(&text)
    }
}

impl Store for FolderStore {
    fn has(&self, id: &Id) -> bool {
        self.path_for(id).exists()
    }

    fn get(&self, id: &Id) -> io::Result<Option<Vec<u8>>> {
        let path = self.path_for(id);
        if !path.exists() {
            return Ok(None);
        }
        fs::read(path).map(Some)
    }

    fn put<R: Read>(&mut self, mut reader: R) -> io::Result<Id> {
        let mut data = Vec::new();
        reader.read_to_end(&mut data)?;
        let id = identify(Cursor::new(&data))?;
        let path = self.path_for(&id);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        if !Path::new(&path).exists() {
            fs::write(path, data)?;
        }
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_round_trips_content() {
        let mut store = MemoryStore::new();
        let id = store.put(Cursor::new(b"hello")).unwrap();
        assert!(store.has(&id));
        assert_eq!(store.get(&id).unwrap(), Some(b"hello".to_vec()));
    }
}
