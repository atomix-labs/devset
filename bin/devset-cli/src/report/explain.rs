//! `explain`: why a file is managed as it is, by each layer that lists it.

use std::io::{self, Write};

use anstyle::Style;
use clap_cargo::style::{GOOD, WARN};
use devset_core::name::ScaffoldId;
use devset_core::profile::Scope;
use devset_core::resolve::Layer;
use devset_core::survey::Scaffolded;
use devset_core::{RelPath, Survey};

use super::{Standing, State};

/// Prints, for `path`, each layer that lists it: how it manages it, its gates, and whether it
/// applies; then where each of its entries stands.
pub(crate) fn explain(survey: &Survey, path: &RelPath) -> io::Result<()> {
    let mut out = anstream::stdout().lock();
    let bold = Style::new().bold();
    writeln!(out, "{bold}{path}{bold:#}")?;
    let resolved = survey.resolved();
    for layer in resolved.layers() {
        let Some(file) = layer.files().get(path) else { continue };
        let spec = &file.spec;
        let part = (spec.scope != Scope::File).then(|| layer.name().as_str());
        let off = file.gated.as_ref().or_else(|| survey.gated(path, part));
        let applies = off.map_or_else(
            || format!("{GOOD}applies{GOOD:#}"),
            |why| format!("{WARN}off: {why}{WARN:#}"),
        );
        writeln!(out, "    {}  {}  {}  {applies}", layer.qualified(), spec.scope, spec.policy())?;
        let when = &spec.when;
        for feature in &when.features {
            let on = layer.features().contains_key(feature);
            writeln!(out, "        when feature {feature}: {}", if on { "on" } else { "off" })?;
        }
        for profile in &when.profiles {
            let on = resolved.layer(profile.as_str()).is_some();
            writeln!(
                out,
                "        when profile {profile}: {}",
                if on { "active" } else { "not active" }
            )?;
        }
        for (name, values) in &when.vars {
            let answer = resolved.answers().get(name).map_or("", String::as_str);
            writeln!(out, "        when {name} is one of {values:?}: {answer:?}")?;
        }
        let outcomes = survey.exists(path, part).unwrap_or_default();
        for pattern in file.exists() {
            let outcome = outcomes.iter().find(|(each, _)| each == pattern);
            let shown = outcome.map_or("unchecked", |&(_, holds)| if holds { "yes" } else { "no" });
            writeln!(out, "        when {pattern} exists: {shown}")?;
        }
        if let Some(group) = &spec.scaffold {
            let id = ScaffoldId { profile: layer.name().clone(), group: group.clone() };
            let decided = survey.scaffolds().get(&id).map_or("undecided", |s| decision(s.decision));
            writeln!(out, "        scaffold {id}: {decided}")?;
        }
        standing(&mut out, survey, layer, path, part)?;
    }
    Ok(())
}

/// Writes where the entry of `layer`'s `part` of `path`, or its whole file, stands.
fn standing(
    out: &mut impl Write, survey: &Survey, layer: &Layer, path: &RelPath, part: Option<&str>,
) -> io::Result<()> {
    let entry = survey.entries().iter().find(|entry| {
        entry.path == *path
            && entry.part.as_ref().map(|p| p.owner.as_str()) == part
            && (part.is_some()
                || survey.resolved().provider(entry).is_none_or(|p| p == layer.name()))
    });
    let Some(entry) = entry else { return Ok(()) };
    let action = match Standing::of(entry) {
        Standing::Pending(change) => format!(", `devset apply` will {change} it"),
        Standing::Drifted(change) => format!(", `devset apply --force` would {change} it"),
        Standing::Conflict => ", conflicted".to_owned(),
        Standing::Local | Standing::InSync => String::new(),
    };
    writeln!(out, "        {}{action}", State::of(entry))
}

/// A scaffold's decision, as people read it.
const fn decision(decided: Scaffolded) -> &'static str {
    match decided {
        Scaffolded::Written => "written",
        Scaffolded::Found => "found the target's own",
    }
}
