//! Template variables: declared by profiles, answered by the target, rendered into payload.

use alloc::borrow::Cow;
use alloc::collections::BTreeMap;
use core::{fmt, str};
use std::collections::HashSet;

use minijinja::{AutoEscape, Environment, UndefinedBehavior};
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};

use crate::errors::{ParseError, Result, VarError};
use crate::profile::PAYLOAD;
use crate::resolve::Layer;
use crate::target::Target;
use crate::tree::Tree;

/// A variable's name: an identifier, `[A-Za-z_][A-Za-z0-9_]*`.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct VarName(Box<str>);

/// A `[vars.name]` entry.
#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VarSpec {
    /// What to ask; the name if unset.
    #[serde(default)]
    pub prompt: Option<String>,
    /// The answer offered.
    #[serde(default)]
    pub default: Option<String>,
}

/// A variable the target has not answered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Question {
    /// The variable.
    pub name: VarName,
    /// What to ask.
    pub prompt: String,
    /// The answer offered: `None` when no layer offers one, or layers offer different ones.
    pub default: Option<String>,
}

impl VarName {
    /// The name as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for VarName {
    type Error = VarError;

    fn try_from(name: String) -> Result<Self, VarError> {
        let mut chars = name.chars();
        let valid = chars
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
        if valid {
            Ok(Self(name.into_boxed_str()))
        } else {
            Err(VarError::Name { name })
        }
    }
}

impl AsRef<str> for VarName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<VarName> for String {
    fn from(name: VarName) -> Self {
        name.0.into_string()
    }
}

impl fmt::Display for VarName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for VarName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&*self.0, f)
    }
}

impl JsonSchema for VarName {
    fn schema_name() -> Cow<'static, str> {
        "VarName".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        String::json_schema(generator)
    }

    fn inline_schema() -> bool {
        true
    }
}

/// What the layers say about one variable, merged.
#[derive(Default)]
struct Declared<'a> {
    /// The first prompt given.
    prompt: Option<&'a str>,
    /// The default all layers agree on.
    default: Offer<'a>,
}

/// A default, as the layers offer it.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Offer<'a> {
    /// No layer offers one.
    #[default]
    None,
    /// Every layer that offers one offers this.
    Agreed(&'a str),
    /// Layers disagree; the target must answer.
    Disputed,
}

/// The target's answer to every variable the layers declare.
///
/// # Errors
/// - [`VarError::Unknown`], the target answers a variable no layer declares.
/// - [`VarError::Unanswered`], with every question, any is unanswered.
pub(crate) fn answers(layers: &[Layer], target: &Target) -> Result<BTreeMap<VarName, String>> {
    let mut declared: BTreeMap<&VarName, Declared<'_>> = BTreeMap::new();
    for (name, spec) in layers.iter().flat_map(Layer::vars) {
        let entry = declared.entry(name).or_default();
        entry.prompt = entry.prompt.or(spec.prompt.as_deref());
        entry.default = match (entry.default, spec.default.as_deref()) {
            (offer, None) => offer,
            (Offer::None, Some(default)) => Offer::Agreed(default),
            (Offer::Agreed(first), Some(default)) if first == default => Offer::Agreed(first),
            (Offer::Agreed(_) | Offer::Disputed, Some(_)) => Offer::Disputed,
        };
    }
    if let Some(name) = target.given().find(|name| !declared.contains_key(name)) {
        let declared = declared.keys().map(|&name| name.clone()).collect();
        return Err(VarError::Unknown {
            name: name.clone(),
            declared,
        }
        .into());
    }
    let mut answers = BTreeMap::new();
    let mut questions = Vec::new();
    for (name, Declared { prompt, default }) in declared {
        match target.answers().get(name) {
            Some(answer) => {
                answers.insert(name.clone(), answer.clone());
            }
            None => questions.push(Question {
                name: name.clone(),
                prompt: prompt.map_or_else(|| name.to_string(), str::to_owned),
                default: match default {
                    Offer::Agreed(default) => Some(default.to_owned()),
                    Offer::None | Offer::Disputed => None,
                },
            }),
        }
    }
    if questions.is_empty() {
        Ok(answers)
    } else {
        Err(VarError::Unanswered { questions }.into())
    }
}

/// `layers` with every template rendered with `answers`.
///
/// # Errors
/// - [`VarError::Undeclared`], a template uses a variable no profile declares.
/// - [`VarError::Template`], a template is not UTF-8.
/// - [`Error::Parse`](crate::Error::Parse), a template does not parse or render, pointing at where.
pub(crate) fn render(
    mut layers: Vec<Layer>,
    answers: &BTreeMap<VarName, String>,
) -> Result<Vec<Layer>> {
    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Strict);
    env.set_keep_trailing_newline(true);
    env.set_auto_escape_callback(|_| AutoEscape::None);
    let globals: HashSet<String> = env.globals().map(|(name, _)| name.to_owned()).collect();
    for layer in &mut layers {
        if !layer.has_templates() {
            continue;
        }
        let mut rendered = Tree::default();
        for (path, bytes) in layer.payload().iter() {
            if !layer.is_template(path) {
                rendered.put(path.clone(), bytes);
                continue;
            }
            let source = str::from_utf8(bytes).map_err(|e| VarError::Template {
                path: path.clone(),
                reason: format!("it is not UTF-8: {e}"),
            })?;
            let file = layer.label(&format!("{PAYLOAD}/{path}"));
            let failed = |e: &minijinja::Error| {
                let message = e.detail().map_or_else(
                    || e.kind().to_string(),
                    |detail| format!("{}: {detail}", e.kind()),
                );
                ParseError {
                    file: file.clone(),
                    text: source.to_owned(),
                    span: e.range(),
                    message,
                }
            };
            let template = env
                .template_from_named_str(path.as_str(), source)
                .map_err(|e| failed(&e))?;
            let mut undeclared: Vec<String> = template
                .undeclared_variables(false)
                .into_iter()
                .filter(|name| {
                    !globals.contains(name) && !answers.keys().any(|v| v.as_str() == name)
                })
                .collect();
            undeclared.sort();
            if let Some(name) = undeclared.into_iter().next() {
                let declared = answers.keys().cloned().collect();
                return Err(VarError::Undeclared {
                    path: path.clone(),
                    name,
                    declared,
                }
                .into());
            }
            let text = template.render(answers).map_err(|e| failed(&e))?;
            rendered.put(path.clone(), text.as_bytes());
        }
        layer.set_payload(rendered);
    }
    Ok(layers)
}

#[cfg(test)]
mod tests {
    use super::VarName;

    #[test]
    fn names_are_identifiers() {
        for name in ["author", "_x", "target_cpu2"] {
            assert!(
                VarName::try_from(name.to_owned()).is_ok(),
                "{name} is valid"
            );
        }
        for name in ["", "2x", "a-b", "a.b", "é"] {
            assert!(
                VarName::try_from(name.to_owned()).is_err(),
                "{name:?} is invalid"
            );
        }
    }
}
