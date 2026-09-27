//! The examples in `examples/`: each README's commands, run in a copy of its directory, print what
//! the README shows. `[..]` in a README matches anything on its line.

use core::fmt::Write as _;
use std::fs;
use std::path::Path;

use snapbox::Assert;

use crate::sandbox::Sandbox;

/// Copies the directory `from` into `to`.
fn copy(from: &Path, to: &Path) {
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let (source, target) = (entry.path(), to.join(entry.file_name()));
        if entry.file_type().unwrap().is_dir() {
            fs::create_dir_all(&target).unwrap();
            copy(&source, &target);
        } else {
            fs::copy(&source, &target).unwrap();
        }
    }
}

/// Each command of the README's `console` blocks, a `$ ` line, with the lines it prints.
fn commands(readme: &str) -> Vec<(String, String)> {
    let mut found: Vec<(String, String)> = Vec::new();
    let mut inside = false;
    for line in readme.lines() {
        if line.starts_with("```") {
            inside = line == "```console";
        } else if !inside {
        } else if let Some(command) = line.strip_prefix("$ ") {
            found.push((command.to_owned(), String::new()));
        } else if let Some((_, printed)) = found.last_mut() {
            printed.push_str(line);
            printed.push('\n');
        }
    }
    found
}

/// Runs the example `name` as its README says, and checks each command prints what it shows.
fn run(name: &str) {
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples").join(name);
    let sandbox = Sandbox::new();
    copy(&example, &sandbox.root);
    let readme = fs::read_to_string(example.join("README.md")).unwrap();
    let commands = commands(&readme);
    assert!(!commands.is_empty(), "{name}'s README shows no command");
    let (mut printed, mut shown) = (String::new(), String::new());
    let mut cwd = String::from(".");
    for (command, expected) in commands {
        writeln!(printed, "$ {command}").unwrap();
        writeln!(shown, "$ {command}").unwrap();
        if let Some(dir) = command.strip_prefix("cd ") {
            cwd = format!("{cwd}/{dir}");
            continue;
        }
        printed.push_str(&sandbox.sh(&cwd, &command));
        shown.push_str(&expected);
    }
    Assert::new().eq(printed, shown);
}

#[test]
fn first_profile() {
    run("first-profile");
}

#[test]
fn local_edits_kept() {
    run("local-edits-kept");
}

#[test]
fn part_of_a_file() {
    run("part-of-a-file");
}

#[test]
fn features_and_gates() {
    run("features-and-gates");
}

#[test]
fn build_on_a_collection() {
    run("build-on-a-collection");
}

#[test]
fn every_example_is_tested() {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut found: Vec<String> = fs::read_dir(examples)
        .unwrap()
        .map(|entry| entry.unwrap())
        .filter(|entry| entry.file_type().unwrap().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    found.sort();
    let tested = [
        "build-on-a-collection",
        "features-and-gates",
        "first-profile",
        "local-edits-kept",
        "part-of-a-file",
    ];
    assert_eq!(found, tested, "each example in examples/ has a test here");
}
