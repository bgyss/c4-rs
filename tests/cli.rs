use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_c4-rs")
}

fn temp_dir(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("c4-rs-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn run(args: &[&str]) -> String {
    let out = Command::new(bin()).args(args).output().unwrap();
    assert!(
        out.status.success(),
        "command failed: {:?}\nstderr: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

fn run_env(args: &[&str], env_key: &str, env_value: &Path) -> String {
    let out = Command::new(bin())
        .args(args)
        .env(env_key, env_value)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "command failed: {:?}\nstderr: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn version_and_stdin_id_work() {
    assert_eq!(run(&["version"]), "c4-rs 0.0.1\n");

    let mut child = Command::new(bin())
        .arg("id")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.as_mut().unwrap().write_all(b"hello").unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8(out.stdout).unwrap().trim(),
        "c447Fm3BJZQ62765jMZJH4m28hrDM7Szbj9CUmj4F4gnvyDYXYz4WfnK2nYRhFvRgYEectEXYBYWLDpLo6XGNAfKdt"
    );
}

#[test]
fn id_cat_store_and_paths_work() {
    let root = temp_dir("id-cat");
    let store = root.join("store");
    let file = root.join("hello.txt");
    fs::write(&file, b"hello").unwrap();

    let out = run_env(&["id", "-s", file.to_str().unwrap()], "C4_STORE", &store);
    assert!(out.contains("hello.txt"));
    let c4id = out
        .split_whitespace()
        .find(|field| field.starts_with("c4") && field.len() == 90)
        .unwrap();
    assert_eq!(run_env(&["cat", c4id], "C4_STORE", &store), "hello");

    let c4m = root.join("hello.c4m");
    fs::write(&c4m, &out).unwrap();
    assert_eq!(run(&["paths", c4m.to_str().unwrap()]), "hello.txt\n");
}

#[test]
fn merge_diff_patch_log_split_and_intersect_work() {
    let root = temp_dir("workflow");
    let a = root.join("a");
    let b = root.join("b");
    fs::create_dir_all(&a).unwrap();
    fs::create_dir_all(&b).unwrap();
    fs::write(a.join("same.txt"), b"old").unwrap();
    fs::write(a.join("only-a.txt"), b"a").unwrap();
    fs::write(b.join("same.txt"), b"new").unwrap();
    fs::write(b.join("only-b.txt"), b"b").unwrap();

    let merged = run(&["merge", a.to_str().unwrap(), b.to_str().unwrap()]);
    assert!(merged.contains("only-a.txt"));
    assert!(merged.contains("only-b.txt"));

    let diff = run(&["diff", a.to_str().unwrap(), b.to_str().unwrap()]);
    assert!(diff.contains("same.txt"));
    assert!(diff.contains("only-b.txt"));

    let chain = root.join("chain.c4m");
    fs::write(&chain, diff).unwrap();
    let patched = run(&["patch", chain.to_str().unwrap()]);
    assert!(patched.contains("same.txt"));

    let log = run(&["log", chain.to_str().unwrap()]);
    assert!(log.contains("(base)") || log.contains("+"));

    let before = root.join("before.c4m");
    let after = root.join("after.c4m");
    run(&[
        "split",
        chain.to_str().unwrap(),
        "1",
        before.to_str().unwrap(),
        after.to_str().unwrap(),
    ]);
    assert!(before.exists());
    assert!(after.exists());

    let intersection = run(&["intersect", a.to_str().unwrap(), b.to_str().unwrap()]);
    assert!(intersection.contains("same.txt"));
}
