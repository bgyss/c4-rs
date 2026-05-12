use crate::c4m::Manifest;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Operation {
    Create(String),
    Delete(String),
    Update(String),
}

pub fn plan(current: &Manifest, target: &Manifest) -> Vec<Operation> {
    let current_paths = manifest_paths(current);
    let target_paths = manifest_paths(target);
    let mut operations = Vec::new();

    for path in target_paths.difference(&current_paths) {
        operations.push(Operation::Create(path.clone()));
    }
    for path in current_paths.difference(&target_paths) {
        operations.push(Operation::Delete(path.clone()));
    }
    for path in current_paths.intersection(&target_paths) {
        let current_entry = current.entries.iter().find(|entry| entry.name == *path);
        let target_entry = target.entries.iter().find(|entry| entry.name == *path);
        if current_entry.and_then(|entry| entry.c4id) != target_entry.and_then(|entry| entry.c4id) {
            operations.push(Operation::Update(path.clone()));
        }
    }
    operations
}

fn manifest_paths(manifest: &Manifest) -> BTreeSet<String> {
    manifest
        .entries
        .iter()
        .map(|entry| entry.name.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::c4m::Entry;
    use crate::id::identify_bytes;

    #[test]
    fn plans_create_delete_update() {
        let mut current = Manifest::new();
        current.add_entry(Entry::file("same.txt", identify_bytes(b"old")));
        current.add_entry(Entry::file("remove.txt", identify_bytes(b"remove")));

        let mut target = Manifest::new();
        target.add_entry(Entry::file("same.txt", identify_bytes(b"new")));
        target.add_entry(Entry::file("add.txt", identify_bytes(b"add")));

        let operations = plan(&current, &target);
        assert!(operations.contains(&Operation::Create("add.txt".to_string())));
        assert!(operations.contains(&Operation::Delete("remove.txt".to_string())));
        assert!(operations.contains(&Operation::Update("same.txt".to_string())));
    }
}
