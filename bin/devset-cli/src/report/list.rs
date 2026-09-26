//! `list`: a source's profiles, each with its description and features.

use std::io::{self, Write};

use anstyle::Style;
use devset_core::collection::Listing;
use devset_core::name::SourceName;
use devset_core::source::Source;

/// Prints `listing`, the profiles of `source`, which the target names `name`.
pub(crate) fn list(
    name: Option<&SourceName>, source: &Source, listing: &Listing,
) -> io::Result<()> {
    let mut out = anstream::stdout().lock();
    let bold = Style::new().bold();
    let meta = listing.meta.as_ref();
    let shown = name.or_else(|| meta.map(|meta| &meta.name));
    match shown {
        Some(shown) => writeln!(out, "{bold}{shown}{bold:#}  {source}")?,
        None => writeln!(out, "{bold}{source}{bold:#}")?,
    }
    if let Some(description) = meta.and_then(|meta| meta.description.as_deref()) {
        writeln!(out, "  {description}")?;
    }
    let width =
        listing.profiles.iter().map(|p| p.manifest.profile.name.as_str().len()).max().unwrap_or(0);
    let mut defaults = false;
    for profile in &listing.profiles {
        let meta = &profile.manifest.profile;
        let description = meta.description.as_deref().unwrap_or_default();
        let line = format!("    {:<width$}  {description}", meta.name.as_str());
        writeln!(out, "{}", line.trim_end())?;
        let features: Vec<String> = profile
            .features()
            .map(|(feature, default)| {
                defaults |= default;
                if default { format!("{feature}*") } else { feature.to_string() }
            })
            .collect();
        if !features.is_empty() {
            writeln!(out, "    {:<width$}  features: {}", "", features.join(", "))?;
        }
    }
    if defaults {
        let dim = Style::new().dimmed();
        writeln!(out, "  {dim}* on by default{dim:#}")?;
    }
    Ok(())
}
