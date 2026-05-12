use crate::c4m::{Entry, FlowDirection, Manifest};
use crate::id::{identify, Id};
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::thread;

const PARALLEL_HASH_THRESHOLD: u64 = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ScanMode {
    Structure,
    Metadata,
    Full,
}

impl ScanMode {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "s" | "1" | "structure" => Some(Self::Structure),
            "m" | "2" | "metadata" => Some(Self::Metadata),
            "f" | "3" | "full" => Some(Self::Full),
            _ => None,
        }
    }
}

pub struct Generator {
    mode: ScanMode,
    excludes: Vec<String>,
}

impl Generator {
    pub fn new(mode: ScanMode) -> Self {
        Self {
            mode,
            excludes: Vec::new(),
        }
    }

    pub fn with_excludes(mut self, excludes: Vec<String>) -> Self {
        self.excludes = excludes;
        self
    }

    pub fn generate_from_path(&self, path: impl AsRef<Path>) -> io::Result<Manifest> {
        let mut manifest = Manifest::new();
        let path = path.as_ref();
        let meta = fs::symlink_metadata(path)?;
        if meta.is_dir() {
            self.scan_dir(path, 0, &mut manifest)?;
        } else {
            manifest.add_entry(self.entry_for(path, &meta, 0)?);
        }
        Ok(manifest)
    }

    fn scan_dir(&self, path: &Path, depth: usize, manifest: &mut Manifest) -> io::Result<Id> {
        let mut children: Vec<PathBuf> = fs::read_dir(path)?
            .map(|entry| entry.map(|e| e.path()))
            .collect::<io::Result<Vec<_>>>()?;
        children.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

        let mut child_infos = Vec::with_capacity(children.len());
        for child in children {
            if self.excluded(&child) {
                continue;
            }
            let meta = fs::symlink_metadata(&child)?;
            child_infos.push((child, meta));
        }

        let file_entries = self.file_entries_for(&child_infos, depth)?;
        let mut child_manifest = Manifest::with_capacity(child_infos.len());
        for (idx, (child, meta)) in child_infos.into_iter().enumerate() {
            if meta.is_dir() {
                let id = self.scan_dir(&child, depth + 1, manifest)?;
                let mut entry = self.base_entry(&child, &meta, depth);
                entry.name.push('/');
                entry.c4id = Some(id);
                child_manifest.add_entry(entry.clone());
                manifest.add_entry(entry);
            } else {
                let entry = file_entries[idx]
                    .clone()
                    .expect("file entry should be precomputed");
                child_manifest.add_entry(entry.clone());
                manifest.add_entry(entry);
            }
        }
        Ok(child_manifest.compute_c4_id())
    }

    fn file_entries_for(
        &self,
        child_infos: &[(PathBuf, fs::Metadata)],
        depth: usize,
    ) -> io::Result<Vec<Option<Entry>>> {
        let mut entries: Vec<Option<Entry>> = std::iter::repeat_with(|| None)
            .take(child_infos.len())
            .collect();
        let files: Vec<_> = child_infos
            .iter()
            .enumerate()
            .filter(|(_, (_, meta))| !meta.is_dir())
            .collect();

        if files.is_empty() {
            return Ok(entries);
        }

        let total_file_bytes: u64 = files.iter().map(|(_, (_, meta))| meta.len()).sum();
        let worker_count = thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1)
            .min(files.len());
        if self.mode != ScanMode::Full
            || worker_count <= 1
            || total_file_bytes < PARALLEL_HASH_THRESHOLD
        {
            for (idx, (path, meta)) in files {
                entries[idx] = Some(self.entry_for(path, meta, depth)?);
            }
            return Ok(entries);
        }

        let chunk_size = (files.len() + worker_count - 1) / worker_count;
        let chunk_results = thread::scope(|scope| {
            let mut handles = Vec::new();
            for chunk in files.chunks(chunk_size) {
                handles.push(scope.spawn(move || -> io::Result<Vec<(usize, Entry)>> {
                    let mut chunk_entries = Vec::with_capacity(chunk.len());
                    for (idx, (path, meta)) in chunk {
                        chunk_entries.push((*idx, self.entry_for(path, meta, depth)?));
                    }
                    Ok(chunk_entries)
                }));
            }

            let mut chunk_results = Vec::with_capacity(handles.len());
            for handle in handles {
                let result = handle.join().map_err(|_| {
                    io::Error::new(io::ErrorKind::Other, "file hashing worker panicked")
                })??;
                chunk_results.push(result);
            }
            Ok::<_, io::Error>(chunk_results)
        })?;

        for chunk in chunk_results {
            for (idx, entry) in chunk {
                entries[idx] = Some(entry);
            }
        }
        Ok(entries)
    }

    fn excluded(&self, path: &Path) -> bool {
        let name = path
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or_default();
        self.excludes
            .iter()
            .any(|pattern| simple_match(pattern, name))
    }

    fn entry_for(&self, path: &Path, meta: &fs::Metadata, depth: usize) -> io::Result<Entry> {
        let mut entry = self.base_entry(path, meta, depth);
        if self.mode == ScanMode::Full {
            entry.c4id = Some(identify(File::open(path)?)?);
        }
        Ok(entry)
    }

    fn base_entry(&self, path: &Path, meta: &fs::Metadata, depth: usize) -> Entry {
        Entry {
            mode: (self.mode >= ScanMode::Metadata).then_some(mode(meta)),
            timestamp: None,
            size: (self.mode >= ScanMode::Metadata).then_some(meta.len() as i64),
            name: path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_string(),
            target: None,
            c4id: None,
            depth,
            hard_link: 0,
            flow_direction: FlowDirection::None,
            flow_target: None,
            is_sequence: false,
        }
    }
}

fn simple_match(pattern: &str, name: &str) -> bool {
    if let Some(suffix) = pattern.strip_prefix("*.") {
        return name.ends_with(&format!(".{suffix}"));
    }
    pattern == name
}

#[cfg(unix)]
fn mode(meta: &fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    let kind = if meta.is_dir() { 0o040000 } else { 0o100000 };
    kind | (meta.permissions().mode() & 0o777)
}

#[cfg(not(unix))]
fn mode(meta: &fs::Metadata) -> u32 {
    if meta.is_dir() {
        0o040755
    } else {
        0o100644
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn scans_single_file() {
        let root = std::env::temp_dir().join(format!("c4-rs-scan-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let file = root.join("hello.txt");
        File::create(&file).unwrap().write_all(b"hello").unwrap();

        let manifest = Generator::new(ScanMode::Full)
            .generate_from_path(&file)
            .unwrap();
        assert_eq!(manifest.entries.len(), 1);
        assert_eq!(manifest.entries[0].name, "hello.txt");
        assert!(manifest.entries[0].c4id.is_some());
        let _ = fs::remove_dir_all(root);
    }
}
