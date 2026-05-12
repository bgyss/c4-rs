use crate::c4m::{Entry, FlowDirection, Manifest};
use crate::id::{identify, Id};
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

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

        let mut child_manifest = Manifest::new();
        for child in children {
            if self.excluded(&child) {
                continue;
            }
            let meta = fs::symlink_metadata(&child)?;
            if meta.is_dir() {
                let id = self.scan_dir(&child, depth + 1, manifest)?;
                let mut entry = self.base_entry(&child, &meta, depth);
                entry.name.push('/');
                entry.c4id = Some(id);
                child_manifest.add_entry(entry.clone());
                manifest.add_entry(entry);
            } else {
                let entry = self.entry_for(&child, &meta, depth)?;
                child_manifest.add_entry(entry.clone());
                manifest.add_entry(entry);
            }
        }
        Ok(child_manifest.compute_c4_id())
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
