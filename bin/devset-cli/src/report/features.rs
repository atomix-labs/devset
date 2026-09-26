//! `features`: each layer's features, who turned each on, and, for one layer, those left off.

use std::io::{self, Write};

use anstyle::Style;
use devset_core::Resolved;
use devset_core::name::ProfileName;
use devset_core::resolve::Layer;

use super::Heading;
use crate::words::list;

/// Prints the features of every layer of `resolved`, or of `only` with the ones it leaves off.
pub(crate) fn features(resolved: &Resolved, only: Option<&ProfileName>) -> io::Result<()> {
    let mut out = anstream::stdout().lock();
    let layers: Vec<&Layer> = only.map_or_else(
        || resolved.layers().iter().collect(),
        |name| resolved.layer(name.as_str()).into_iter().collect(),
    );
    if layers.is_empty() {
        let name = only.map_or_else(String::new, ToString::to_string);
        return writeln!(out, "No layer is named {name}.");
    }
    for layer in layers {
        writeln!(out, "{}", Heading(layer))?;
        let width = layer.declared().iter().map(|f| f.as_str().len()).max().unwrap_or(0);
        for (feature, by) in layer.features() {
            let by: Vec<String> = by.iter().map(ToString::to_string).collect();
            writeln!(out, "    {feature:<width$}  <- {}", by.join(", "))?;
        }
        let off: Vec<_> =
            layer.declared().iter().filter(|f| !layer.features().contains_key(*f)).collect();
        if only.is_some() && !off.is_empty() {
            let dim = Style::new().dimmed();
            writeln!(out, "    {dim}off: {}{dim:#}", list(&off))?;
        }
    }
    Ok(())
}
