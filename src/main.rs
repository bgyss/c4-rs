use c4::c4m::{format_manifest, parse_manifest, parse_manifest_chain, Entry, Manifest};
use c4::id::parse as parse_id;
use c4::scan::{Generator, ScanMode};
use c4::store::{FolderStore, Store};
use c4::{identify, Id};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;

const VERSION: &str = "0.0.1";

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("version") => {
            println!("c4-rs {VERSION}");
            Ok(())
        }
        Some("id") => run_id(&args[1..]),
        Some("cat") => run_cat(&args[1..]),
        Some("paths") => run_paths(&args[1..]),
        Some("merge") => run_merge(&args[1..]),
        Some("diff") => run_diff(&args[1..]),
        Some("patch") => run_patch(&args[1..]),
        Some("log") => run_log(&args[1..]),
        Some("split") => run_split(&args[1..]),
        Some("intersect") => run_intersect(&args[1..]),
        Some("explain") => run_explain(&args[1..]),
        Some("--help") | Some("-h") | None => {
            usage();
            Ok(())
        }
        Some(path) if Path::new(path).exists() => run_id(&args),
        Some(_) => {
            usage();
            std::process::exit(1);
        }
    }
}

fn run_id(args: &[String]) -> io::Result<()> {
    let parsed = parse_common_flags(args);
    if parsed.paths.is_empty() {
        let id = identify(io::stdin().lock())?;
        println!("{id}");
        return Ok(());
    }

    for path in &parsed.paths {
        let path = Path::new(path);
        if path.extension().and_then(|v| v.to_str()) == Some("c4m") {
            let data = fs::read_to_string(path)?;
            let formatted = format_manifest(&data).map_err(invalid_data)?;
            if parsed.store {
                store_bytes(formatted.as_bytes())?;
            }
            print!("{formatted}");
            continue;
        }

        let generator = Generator::new(parsed.mode).with_excludes(parsed.excludes.clone());
        let manifest = generator.generate_from_path(path)?;
        if parsed.store {
            store_path_content(path)?;
        }
        print!("{}", manifest.canonical());
    }
    Ok(())
}

fn run_cat(args: &[String]) -> io::Result<()> {
    let parsed = parse_common_flags(args);
    let Some(target) = parsed.paths.first() else {
        return usage_err("c4-rs cat [-e] [-r] <c4id|path>");
    };
    let path = Path::new(target);
    if path.exists() {
        let data = fs::read(path)?;
        if looks_like_c4m(&data) {
            let text = String::from_utf8_lossy(&data);
            print!("{}", format_manifest(&text).map_err(invalid_data)?);
        } else {
            io::stdout().write_all(&data)?;
        }
        return Ok(());
    }

    let id = parse_id(target).map_err(invalid_data)?;
    let store_dir = env::var("C4_STORE")
        .map_err(|_| io::Error::new(io::ErrorKind::NotFound, "C4_STORE is not set"))?;
    let store = FolderStore::new(store_dir);
    let data = store
        .get(&id)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "content not found"))?;
    io::stdout().write_all(&data)?;
    Ok(())
}

fn run_paths(args: &[String]) -> io::Result<()> {
    let input = read_argument_or_stdin(args.first().map(String::as_str))?;
    if looks_like_c4m(input.as_bytes()) {
        let manifest = parse_manifest(&input).map_err(invalid_data)?;
        for path in manifest.entry_paths() {
            println!("{path}");
        }
    } else {
        let paths: Vec<String> = input
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect();
        print!("{}", manifest_from_paths(&paths).canonical());
    }
    Ok(())
}

fn run_merge(args: &[String]) -> io::Result<()> {
    let parsed = parse_common_flags(args);
    if parsed.paths.len() < 2 {
        return usage_err("c4-rs merge <path>...");
    }
    let mut entries = BTreeMap::new();
    for path in &parsed.paths {
        for entry in load_manifest_or_dir(path, parsed.mode)?.entries {
            entries.entry(entry.name.clone()).or_insert(entry);
        }
    }
    let mut manifest = Manifest::new();
    for entry in entries.into_values() {
        manifest.add_entry(entry);
    }
    print!("{}", manifest.canonical());
    Ok(())
}

fn run_diff(args: &[String]) -> io::Result<()> {
    let parsed = parse_common_flags(args);
    if parsed.paths.len() != 2 {
        return usage_err("c4-rs diff <old> <new>");
    }
    let (left, right) = if parsed.reverse {
        (&parsed.paths[1], &parsed.paths[0])
    } else {
        (&parsed.paths[0], &parsed.paths[1])
    };
    let old = load_manifest_or_dir(left, parsed.mode)?;
    let new = load_manifest_or_dir(right, parsed.mode)?;
    if old.canonical() == new.canonical() {
        return Ok(());
    }

    println!("{}", old.compute_c4_id());
    let old_map = entry_map(&old);
    for entry in &new.entries {
        if old_map.get(&entry.name).and_then(|e| e.c4id) != entry.c4id {
            println!("{}", entry.canonical());
        }
    }
    println!("{}", new.compute_c4_id());
    Ok(())
}

fn run_patch(args: &[String]) -> io::Result<()> {
    let Some(path) = args.first() else {
        return usage_err("c4-rs patch <target>");
    };
    let data = fs::read_to_string(path)?;
    let manifests = parse_manifest_chain(&data).map_err(invalid_data)?;
    if let Some(last) = manifests.last() {
        print!("{}", last.canonical());
    }
    Ok(())
}

fn run_log(args: &[String]) -> io::Result<()> {
    let Some(path) = args.first() else {
        return usage_err("c4-rs log <file.c4m>");
    };
    let data = fs::read_to_string(path)?;
    let manifests = parse_manifest_chain(&data).map_err(invalid_data)?;
    for (idx, manifest) in manifests.iter().enumerate() {
        if idx == 0 {
            println!("{} (base)", manifest.compute_c4_id());
        } else {
            println!("{} +{}", manifest.compute_c4_id(), manifest.entries.len());
        }
    }
    Ok(())
}

fn run_split(args: &[String]) -> io::Result<()> {
    if args.len() != 4 {
        return usage_err("c4-rs split <file.c4m> <N> <before.c4m> <after.c4m>");
    }
    let data = fs::read_to_string(&args[0])?;
    let manifests = parse_manifest_chain(&data).map_err(invalid_data)?;
    let split_at = args[1].parse::<usize>().unwrap_or(1).min(manifests.len());
    let before = manifests[..split_at]
        .iter()
        .map(Manifest::canonical)
        .collect::<String>();
    let after = manifests[split_at..]
        .iter()
        .map(Manifest::canonical)
        .collect::<String>();
    fs::write(&args[2], before)?;
    fs::write(&args[3], after)?;
    Ok(())
}

fn run_intersect(args: &[String]) -> io::Result<()> {
    let parsed = parse_common_flags(args);
    if parsed.paths.len() < 2 {
        return usage_err("c4-rs intersect <a> <b>");
    }
    let a = load_manifest_or_dir(&parsed.paths[parsed.paths.len() - 2], parsed.mode)?;
    let b = load_manifest_or_dir(&parsed.paths[parsed.paths.len() - 1], parsed.mode)?;
    let b_names: BTreeSet<_> = b.entries.iter().map(|entry| entry.name.as_str()).collect();
    let mut out = Manifest::new();
    for entry in a.entries {
        if b_names.contains(entry.name.as_str()) {
            out.add_entry(entry);
        }
    }
    print!("{}", out.canonical());
    Ok(())
}

fn run_explain(args: &[String]) -> io::Result<()> {
    match args.first().map(String::as_str) {
        Some("id") if args.len() >= 2 => {
            let manifest = load_manifest_or_dir(&args[1], ScanMode::Full)?;
            println!(
                "Scanning {}: {} entries, {} files",
                args[1],
                manifest.entries.len(),
                manifest.entries.iter().filter(|e| !e.is_dir()).count()
            );
        }
        Some("diff") if args.len() >= 3 => {
            let old = load_manifest_or_dir(&args[1], ScanMode::Full)?;
            let new = load_manifest_or_dir(&args[2], ScanMode::Full)?;
            println!(
                "Comparing {} against {}: {} -> {} entries",
                args[1],
                args[2],
                old.entries.len(),
                new.entries.len()
            );
        }
        Some("patch") if args.len() >= 2 => {
            let manifest = load_manifest_or_dir(&args[1], ScanMode::Full)?;
            println!("Patch target contains {} entries.", manifest.entries.len());
        }
        _ => usage(),
    }
    Ok(())
}

#[derive(Clone)]
struct ParsedFlags {
    paths: Vec<String>,
    mode: ScanMode,
    store: bool,
    reverse: bool,
    excludes: Vec<String>,
}

fn parse_common_flags(args: &[String]) -> ParsedFlags {
    let mut parsed = ParsedFlags {
        paths: Vec::new(),
        mode: ScanMode::Full,
        store: false,
        reverse: false,
        excludes: Vec::new(),
    };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-s" | "--store" => parsed.store = true,
            "-r" | "--reverse" => parsed.reverse = true,
            "-e" | "--ergonomic" | "-S" | "--sequence" => {}
            "-m" | "--mode" => {
                if let Some(value) = args.get(i + 1).and_then(|v| ScanMode::parse(v)) {
                    parsed.mode = value;
                    i += 1;
                }
            }
            "--exclude" => {
                if let Some(value) = args.get(i + 1) {
                    parsed.excludes.push(value.clone());
                    i += 1;
                }
            }
            arg if arg.starts_with("-m") && arg.len() > 2 => {
                if let Some(value) = ScanMode::parse(&arg[2..]) {
                    parsed.mode = value;
                }
            }
            arg if arg.starts_with('-') && arg.len() > 2 => {
                for ch in arg[1..].chars() {
                    match ch {
                        's' => parsed.store = true,
                        'r' => parsed.reverse = true,
                        'e' | 'S' => {}
                        _ => {}
                    }
                }
            }
            arg => parsed.paths.push(arg.to_string()),
        }
        i += 1;
    }
    parsed
}

fn load_manifest_or_dir(path: &str, mode: ScanMode) -> io::Result<Manifest> {
    if path == "-" {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;
        return parse_manifest(&input).map_err(invalid_data);
    }
    let meta = fs::symlink_metadata(path)?;
    if meta.is_dir() {
        return Generator::new(mode).generate_from_path(path);
    }
    let data = fs::read_to_string(path)?;
    parse_manifest(&data).map_err(invalid_data)
}

fn entry_map(manifest: &Manifest) -> BTreeMap<String, Entry> {
    manifest
        .entries
        .iter()
        .map(|entry| (entry.name.clone(), entry.clone()))
        .collect()
}

fn manifest_from_paths(paths: &[String]) -> Manifest {
    let mut all = BTreeSet::new();
    for path in paths {
        all.insert(path.clone());
        let mut parts: Vec<&str> = path.trim_end_matches('/').split('/').collect();
        while parts.len() > 1 {
            parts.pop();
            all.insert(format!("{}/", parts.join("/")));
        }
    }
    let mut manifest = Manifest::new();
    for path in all {
        let is_dir = path.ends_with('/');
        let clean = path.trim_end_matches('/');
        let depth = clean.matches('/').count();
        let name = clean.rsplit('/').next().unwrap_or(clean);
        manifest.add_entry(Entry {
            mode: None,
            timestamp: None,
            size: None,
            name: if is_dir {
                format!("{name}/")
            } else {
                name.to_string()
            },
            target: None,
            c4id: None,
            depth,
            hard_link: 0,
            flow_direction: c4::c4m::FlowDirection::None,
            flow_target: None,
            is_sequence: false,
        });
    }
    manifest
}

fn store_path_content(path: &Path) -> io::Result<()> {
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let child = entry?.path();
            store_path_content(&child)?;
        }
        return Ok(());
    }
    store_bytes(&fs::read(path)?)?;
    Ok(())
}

fn store_bytes(data: &[u8]) -> io::Result<Id> {
    let store_dir = env::var("C4_STORE").unwrap_or_else(|_| ".c4".to_string());
    let mut store = FolderStore::new(store_dir);
    store.put(data)
}

fn read_argument_or_stdin(arg: Option<&str>) -> io::Result<String> {
    if let Some(path) = arg.filter(|value| *value != "-") {
        fs::read_to_string(path)
    } else {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;
        Ok(input)
    }
}

fn looks_like_c4m(data: &[u8]) -> bool {
    let text = String::from_utf8_lossy(data);
    text.lines().any(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with("- ")
            || trimmed.starts_with("-r")
            || trimmed.starts_with("d")
            || trimmed.starts_with("l")
    })
}

fn invalid_data(err: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, err.to_string())
}

fn usage_err(message: &str) -> io::Result<()> {
    eprintln!("Usage: {message}");
    std::process::exit(1);
}

fn usage() {
    eprintln!(
        "c4-rs - Rust C4 port\n\nUsage:\n  c4-rs id <path>...\n  c4-rs cat <c4id|path>\n  c4-rs diff <old> <new>\n  c4-rs patch <target>\n  c4-rs merge <path>...\n  c4-rs paths [file|-]\n  c4-rs log <file.c4m>\n  c4-rs split <file.c4m> <N> <before> <after>\n  c4-rs intersect <a> <b>\n  c4-rs explain <command> ...\n  c4-rs version"
    );
}
