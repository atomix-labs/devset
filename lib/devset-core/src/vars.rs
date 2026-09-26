//! Template variables: declared by profiles, answered by the target, rendered into payload.

use alloc::collections::BTreeMap;
use core::str;
use std::collections::HashSet;

use derive_more::{AsRef, Display, Into};
use minijinja::{AutoEscape, Environment, UndefinedBehavior, Value};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::errors::{ParseError, ProfileError, Result, VarError};
use crate::gate::Pattern;
use crate::path::RelPath;
use crate::profile::{MANIFEST, PAYLOAD};
use crate::resolve::{Layer, LayerFile};
use crate::target::Target;
use crate::tree::Tree;

/// The name templates see the target's profiles under, which no variable may take.
pub(crate) const RESERVED: &str = "devset";

/// A variable's name: an identifier, `[A-Za-z_][A-Za-z0-9_]*`, but not `devset`.
#[derive(
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    AsRef,
    Display,
    derive_more::Debug,
    Into,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[as_ref(str)]
#[debug("{_0:?}")]
#[into(String)]
#[serde(try_from = "String", into = "String")]
#[schemars(inline)]
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
        let valid = chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
        match (valid, name.as_str()) {
            (false, _) => Err(VarError::Name { name }),
            (true, RESERVED) => Err(VarError::Reserved),
            (true, _) => Ok(Self(name.into_boxed_str())),
        }
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
        return Err(VarError::Unknown { name: name.clone(), declared }.into());
    }
    let mut answers = BTreeMap::new();
    let mut questions = Vec::new();
    for (name, Declared { prompt, default }) in declared {
        match target.answers().get(name) {
            Some(answer) => {
                answers.insert(name.clone(), answer.clone());
            },
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
    if questions.is_empty() { Ok(answers) } else { Err(VarError::Unanswered { questions }.into()) }
}

/// The graph, as templates see it under `devset`.
#[derive(Debug, Serialize)]
pub(crate) struct Context {
    /// The target directory's name.
    target: String,
    /// Every active profile's name.
    profiles: Vec<String>,
    /// Each layer, in the order they apply.
    layers: Vec<LayerContext>,
}

/// A layer, as templates see it in `devset.layers`.
#[derive(Debug, Serialize)]
struct LayerContext {
    /// Its profile's name.
    profile: String,
    /// Its source's name, or where it is for a source a profile requires by `git`.
    source: String,
    /// Its features that are on.
    features: Vec<String>,
    /// Whether the target configures it, rather than a profile requiring it.
    configured: bool,
}

/// What one layer's templates see under `devset`: the graph, and the layer's own features.
#[derive(Serialize)]
struct Devset<'a> {
    /// The target directory's name.
    target: &'a str,
    /// Every active profile's name.
    profiles: &'a [String],
    /// Each layer.
    layers: &'a [LayerContext],
    /// This layer's features that are on.
    features: &'a [String],
}

impl Context {
    /// The graph of `layers`, applied to `target`.
    pub(crate) fn of(target: &Target, layers: &[Layer]) -> Self {
        let layers: Vec<LayerContext> = layers
            .iter()
            .map(|layer| LayerContext {
                profile: layer.name().to_string(),
                source: layer
                    .source_name()
                    .map_or_else(|| layer.source().to_string(), ToString::to_string),
                features: layer.features().keys().map(ToString::to_string).collect(),
                configured: layer.configured(),
            })
            .collect();
        Self {
            target: target.root().file_name().unwrap_or_default().to_owned(),
            profiles: layers.iter().map(|layer| layer.profile.clone()).collect(),
            layers,
        }
    }

    /// What `layer`'s templates see: `answers`, and `devset`.
    fn vars(&self, layer: &Layer, answers: &BTreeMap<VarName, String>) -> Value {
        let empty = Vec::new();
        let features = self
            .layers
            .iter()
            .find(|l| *layer.name() == *l.profile)
            .map_or(&empty, |l| &l.features);
        let devset = Devset {
            target: &self.target,
            profiles: &self.profiles,
            layers: &self.layers,
            features,
        };
        let mut vars: BTreeMap<&str, Value> = answers
            .iter()
            .map(|(name, answer)| (name.as_str(), Value::from(answer.as_str())))
            .collect();
        vars.insert(RESERVED, Value::from_serialize(&devset));
        Value::from_serialize(&vars)
    }
}

/// Renders templates for one layer: its answers and `devset` in scope, a variable nothing
/// declares refused.
struct Renderer<'a> {
    /// The environment: strict, unescaped, trailing newlines kept.
    env: Environment<'static>,
    /// What the templates see.
    vars: Value,
    /// The names templates may use: the answers', `devset`, and the environment's globals.
    known: HashSet<String>,
    /// Every declared variable, for a message.
    declared: &'a BTreeMap<VarName, String>,
}

impl<'a> Renderer<'a> {
    /// A renderer for `layer`.
    fn new(layer: &Layer, answers: &'a BTreeMap<VarName, String>, context: &Context) -> Self {
        let mut env = Environment::new();
        env.set_undefined_behavior(UndefinedBehavior::Strict);
        env.set_keep_trailing_newline(true);
        env.set_auto_escape_callback(|_| AutoEscape::None);
        let mut known: HashSet<String> = env.globals().map(|(name, _)| name.to_owned()).collect();
        known.extend(answers.keys().map(ToString::to_string));
        known.insert(RESERVED.to_owned());
        Self { env, vars: context.vars(layer, answers), known, declared: answers }
    }

    /// `source`, a template named `name` in `file`, rendered.
    ///
    /// # Errors
    /// - [`VarError::Undeclared`], it uses a variable no profile declares.
    /// - [`Error::Parse`](crate::Error::Parse), it does not parse or render, pointing at where.
    fn render(&self, name: &RelPath, source: &str, file: &str) -> Result<String> {
        let failed = |e: &minijinja::Error| {
            let message = e
                .detail()
                .map_or_else(|| e.kind().to_string(), |detail| format!("{}: {detail}", e.kind()));
            ParseError { file: file.to_owned(), text: source.to_owned(), span: e.range(), message }
        };
        let template =
            self.env.template_from_named_str(name.as_str(), source).map_err(|e| failed(&e))?;
        let undeclared = template
            .undeclared_variables(false)
            .into_iter()
            .filter(|name| !self.known.contains(name))
            .min();
        if let Some(undeclared) = undeclared {
            let declared = self.declared.keys().cloned().collect();
            return Err(
                VarError::Undeclared { path: name.clone(), name: undeclared, declared }.into()
            );
        }
        Ok(template.render(&self.vars).map_err(|e| failed(&e))?)
    }

    /// `written`, a path or a pattern in `file`, its variables answered; unchanged without any.
    fn text(&self, written: &str, file: &str) -> Result<String> {
        if !written.contains('{') {
            return Ok(written.to_owned());
        }
        let name = RelPath::new(MANIFEST)?;
        self.render(&name, written, file)
    }
}

/// Renders each layer's paths, the paths and globs its gates name, and its scaffolds' sentinels.
///
/// # Errors
/// - [`ProfileError::Manifest`], a path renders to one that is not a path in the target.
/// - [`ProfileError::SamePath`], two of a layer's paths render to one.
/// - [`ProfileError::Pattern`], a pattern renders to neither a path nor a glob.
/// - As [`render`], a variable is undeclared, or a template does not parse.
pub(crate) fn render_paths(
    layers: &mut [Layer], answers: &BTreeMap<VarName, String>, context: &Context,
) -> Result<()> {
    for layer in layers {
        let (specs, scaffolds) = layer.take_specs();
        let templates = Renderer::new(layer, answers, context);
        let file = layer.label(MANIFEST);
        let pattern = |written: &str| -> Result<Pattern> {
            let rendered = templates.text(written, &file)?;
            Pattern::parse(&rendered).map_err(|reason| {
                ProfileError::Pattern { profile: layer.name().clone(), pattern: rendered, reason }
                    .into()
            })
        };
        let mut files: BTreeMap<RelPath, LayerFile> = BTreeMap::new();
        for (written, spec) in specs {
            let rendered = templates.text(written.as_str(), &file)?;
            let path = RelPath::new(&rendered).map_err(|e| ProfileError::Manifest {
                file: file.clone(),
                message: format!("[files.\"{written}\"] is {rendered:?} once answered, which is not a path in the target: {e}"),
            })?;
            let exists =
                spec.when.exists.iter().map(|each| pattern(each)).collect::<Result<_>>()?;
            let entry = LayerFile { written: written.clone(), spec, exists, gated: None };
            if let Some(first) = files.insert(path.clone(), entry) {
                let (profile, first) = (layer.name().clone(), first.written);
                return Err(ProfileError::SamePath { profile, path, first, second: written }.into());
            }
        }
        let scaffolds = scaffolds
            .into_iter()
            .map(|(name, spec)| {
                Ok((name, spec.unless.0.iter().map(|each| pattern(each)).collect::<Result<_>>()?))
            })
            .collect::<Result<_>>()?;
        layer.set_files(files, scaffolds);
    }
    Ok(())
}

/// Renders the payload of every file that applies, and every starter, with `answers` and the
/// graph; digests each layer.
///
/// # Errors
/// - [`VarError::Undeclared`], a template uses a variable no profile declares.
/// - [`VarError::Template`], a template is not UTF-8.
/// - [`Error::Parse`](crate::Error::Parse), a template does not parse or render, pointing at where.
pub(crate) fn render(
    layers: &mut [Layer], answers: &BTreeMap<VarName, String>, context: &Context,
) -> Result<()> {
    for layer in layers {
        let templates = Renderer::new(layer, answers, context);
        let (mut payload, mut starters) = (Tree::default(), Tree::default());
        let bytes_of = |written: &RelPath, template: bool| -> Result<Vec<u8>> {
            let bytes = layer.written().get(written).unwrap_or_default();
            if !template {
                return Ok(bytes.to_vec());
            }
            let source = str::from_utf8(bytes).map_err(|e| VarError::Template {
                path: written.clone(),
                reason: format!("it is not UTF-8: {e}"),
            })?;
            let file = layer.label(&format!("{PAYLOAD}/{written}"));
            Ok(templates.render(written, source, &file)?.into_bytes())
        };
        for (path, file) in layer.files().iter().filter(|(_, file)| file.gated.is_none()) {
            payload.put(path.clone(), &bytes_of(&file.written, file.spec.template)?);
            if let Some(starter) = &file.spec.starter {
                starters.put(path.clone(), &bytes_of(starter, file.spec.template)?);
            }
        }
        layer.set_payload(payload, starters);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::VarName;

    #[test]
    fn names_are_identifiers() {
        for name in ["author", "_x", "target_cpu2"] {
            assert!(VarName::try_from(name.to_owned()).is_ok(), "{name} is valid");
        }
        for name in ["", "2x", "a-b", "a.b", "é", "devset"] {
            assert!(VarName::try_from(name.to_owned()).is_err(), "{name:?} is invalid");
        }
    }
}
