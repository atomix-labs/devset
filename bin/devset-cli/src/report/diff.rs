//! `diff`: how each managed file differs from the profile's version.

use core::str;
use std::io::{self, Write};

use devset_core::name::ProfileName;
use devset_core::{Error, RelPath, Survey, Target};
use diffy::{DiffOptions, PatchFormatter};

/// Prints how each managed file, or each of `paths`, differs from the profile's version.
///
/// A unified diff from the profile's lines to yours; a file that matches prints nothing, even
/// when its whitespace differs. A part is shown as its whole file, the part as the profile has
/// it, so the diff is the part's alone.
pub(crate) fn diff(survey: &Survey, target: &Target, paths: &[RelPath]) -> Result<(), Error> {
    let resolved = survey.resolved();
    let formatter = PatchFormatter::new().with_color();
    let mut out = anstream::stdout().lock();
    for entry in survey.entries() {
        if !paths.is_empty() && !paths.contains(&entry.path) {
            continue;
        }
        let Some(want) = entry.want else { continue };
        if entry.found.is_some_and(|found| found.same_content(&want.fingerprint)) {
            continue;
        }
        let Some(profile) = survey.wanted(entry, target)? else {
            continue;
        };
        let disk = match fs_err::read(entry.path.under(target.root())) {
            Ok(disk) => Some(disk),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        let yours = match disk {
            Some(_) => format!("{}  (yours)", entry.path),
            None => "/dev/null".to_owned(),
        };
        let disk = disk.unwrap_or_default();
        let (Ok(profile), Ok(disk)) = (text(&profile), text(&disk)) else {
            writeln!(out, "Binary files differ: {entry}")?;
            continue;
        };
        let layer = resolved.provider(entry).map_or("", ProfileName::as_str);
        let patch = DiffOptions::new()
            .set_original_filename(format!("{}  (profile {layer})", entry.path))
            .set_modified_filename(yours)
            .create_patch(profile, disk);
        write!(out, "{}", formatter.fmt_patch(&patch))?;
    }
    Ok(())
}

/// `bytes` as text, unless they are binary by git's test or not UTF-8.
fn text(bytes: &[u8]) -> Result<&str, ()> {
    if bytes.get(..8000).unwrap_or(bytes).contains(&0) {
        return Err(());
    }
    str::from_utf8(bytes).map_err(drop)
}
