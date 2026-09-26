//! The profile graph: every active profile, the features the target and its requirers turn on, and
//! the order the profiles apply in.
//!
//! Features are Cargo's. They only add, so the graph is expanded until nothing changes: a request
//! for a profile turns on the features it names, and each feature turned on may turn on more, in
//! the profile or in what it requires. A profile reached twice is one node, its features the union.
//! The order is topological, each requirement before its requirers.

use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
use core::fmt;
use std::collections::HashMap;

use crate::errors::{ProfileError, Result};
use crate::name::{FeatureName, ProfileName};
use crate::profile::{FeatureRef, Manifest};
use crate::source::Source;

/// Who turned a feature on.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Enabler {
    /// The target, in the layer's `features`.
    Target,
    /// The profile's `default` list.
    Default,
    /// A profile that requires it, in its `[requires]` entry.
    Requires(ProfileName),
    /// A feature: of a profile that requires it, or of the profile itself.
    Feature {
        /// The profile the feature is of.
        profile: ProfileName,
        /// The feature.
        feature: FeatureName,
    },
}

/// `target`, `default`, `rust`, or `rust/docs`.
impl fmt::Display for Enabler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Target => f.write_str("target"),
            Self::Default => f.write_str("default"),
            Self::Requires(profile) => write!(f, "{profile}"),
            Self::Feature { profile, feature } => write!(f, "{profile}/{feature}"),
        }
    }
}

/// Something the graph settled that the target may not expect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Warning {
    /// A layer sets `default-features = false`, and a profile that requires it turns them on.
    DefaultsOn {
        /// The layer.
        profile: ProfileName,
        /// The profile that turns them on.
        by: ProfileName,
    },
}

/// How the graph reads a profile.
pub(crate) trait Load {
    /// A profile, read.
    type Profile;

    /// The profile named `name` in `source`.
    fn load(&mut self, source: &Source, name: &ProfileName) -> Result<Self::Profile>;

    /// What `profile` declares.
    fn manifest(profile: &Self::Profile) -> &Manifest;
}

/// A layer the target configures: its profile, where, and the features it turns on.
#[derive(Clone, Debug)]
pub(crate) struct Configured<'a> {
    /// The profile's source.
    pub source: &'a Source,
    /// The profile.
    pub name: &'a ProfileName,
    /// The features the target turns on.
    pub features: &'a [FeatureName],
    /// Whether its default features are on.
    pub defaults: bool,
}

/// One active profile.
#[derive(Debug)]
pub(crate) struct Node<P> {
    /// The profile, read.
    pub profile: P,
    /// Its name.
    pub name: ProfileName,
    /// Its source.
    pub source: Source,
    /// Every feature that is on, with who turned it on.
    pub features: BTreeMap<FeatureName, BTreeSet<Enabler>>,
    /// The profiles that require it and activated it.
    pub required_by: BTreeSet<ProfileName>,
    /// Whether the target configures it as a layer.
    pub configured: bool,
    /// Whether its default features are on.
    defaults: bool,
    /// Whether the target switched its defaults off.
    no_defaults: bool,
    /// Its requirements that are active.
    active: BTreeSet<ProfileName>,
}

/// Every active profile, in the order they apply, and what the target should hear about.
#[derive(Debug)]
pub(crate) struct Graph<P> {
    /// Each requirement before its requirers; otherwise as configured.
    pub nodes: Vec<Node<P>>,
    /// What the graph settled that the target may not expect.
    pub warnings: Vec<Warning>,
}

/// A request for a profile: to activate it, and to turn features on.
struct Request {
    /// Where it is; `None` for a weak request, which only reaches the profile already active.
    source: Option<Source>,
    /// Its name.
    name: ProfileName,
    /// Who asks: the target, or a profile.
    by: Option<ProfileName>,
    /// The features to turn on, each with who turns it on.
    features: Vec<(FeatureName, Enabler)>,
    /// Whether to turn its default features on.
    defaults: bool,
}

/// A feature to turn on, and who turns it on.
type Turn = (FeatureName, Enabler);

/// What a profile asks of one of its requirements.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Ask {
    /// The requirement.
    dep: ProfileName,
    /// A feature to turn on in it, beside what its `[requires]` entry asks.
    turn: Option<Turn>,
    /// Whether it only reaches the requirement through a weak feature: it activates nothing.
    weak: bool,
}

impl<P> Graph<P> {
    /// The graph of `configured`, the target's layers in order, read through `load`.
    ///
    /// # Errors
    /// - [`ProfileError::UnknownFeature`], a feature is asked of a profile that lacks it.
    /// - [`ProfileError::Cycle`], requirements lead back to a profile that requires them.
    /// - [`ProfileError::SameName`] or [`ProfileError::Diverged`], two profiles have one name.
    /// - Whatever `load` fails with.
    pub(crate) fn build<L: Load<Profile = P>>(
        configured: &[Configured<'_>], load: &mut L,
    ) -> Result<Self> {
        let mut builder = Builder {
            load,
            nodes: Vec::new(),
            index: HashMap::new(),
            queue: VecDeque::new(),
            warnings: Vec::new(),
            changes: 0,
        };
        for layer in configured {
            let features = layer.features.iter().map(|f| (f.clone(), Enabler::Target)).collect();
            builder.queue.push_back(Request {
                source: Some(layer.source.clone()),
                name: layer.name.clone(),
                by: None,
                features,
                defaults: layer.defaults,
            });
        }
        // A weak feature waits for its profile: expanded again once nothing else changes, until
        // nothing does.
        loop {
            while let Some(request) = builder.queue.pop_front() {
                builder.serve(request)?;
            }
            let seen = builder.changes;
            for at in 0..builder.nodes.len() {
                builder.expand(at);
            }
            while let Some(request) = builder.queue.pop_front() {
                builder.serve(request)?;
            }
            if builder.changes == seen {
                break;
            }
        }
        let order = builder.order(configured)?;
        let Builder { nodes, warnings, .. } = builder;
        let mut nodes: Vec<Option<Node<P>>> = nodes.into_iter().map(Some).collect();
        let nodes = order.into_iter().filter_map(|at| nodes.get_mut(at)?.take()).collect();
        Ok(Self { nodes, warnings })
    }
}

/// The graph being built.
struct Builder<'l, L: Load> {
    /// How profiles are read.
    load: &'l mut L,
    /// Every active profile, in the order met.
    nodes: Vec<Node<L::Profile>>,
    /// Each node's position, by name.
    index: HashMap<ProfileName, usize>,
    /// Requests not yet served.
    queue: VecDeque<Request>,
    /// What the target should hear about.
    warnings: Vec<Warning>,
    /// How many times a node has changed, so the builder knows when the graph settles.
    changes: usize,
}

impl<L: Load> Builder<'_, L> {
    /// Serves `request`: the profile activated, its features turned on, and what they turn on
    /// requested in turn.
    fn serve(&mut self, request: Request) -> Result<()> {
        let Request { source, name, by, features, defaults } = request;
        let weak = source.is_none();
        let met = self.index.get(&name).and_then(|&at| self.nodes.get(at));
        let Some(source) = source.or_else(|| met.map(|met| met.source.clone())) else {
            return Ok(());
        };
        let (at, new) = self.node(&source, &name)?;
        let Some(node) = self.nodes.get_mut(at) else { return Ok(()) };
        let mut changed = new;
        match &by {
            None => {
                node.configured = true;
                node.no_defaults = !defaults;
            },
            Some(requirer) if !weak => {
                node.required_by.insert(requirer.clone());
            },
            Some(_) => {},
        }
        let declared = &L::manifest(&node.profile).features;
        if defaults && !node.defaults {
            node.defaults = true;
            if let (true, Some(by)) = (node.no_defaults, &by) {
                self.warnings.push(Warning::DefaultsOn { profile: name.clone(), by: by.clone() });
            }
            for feature in &declared.default {
                changed |=
                    node.features.entry(feature.clone()).or_default().insert(Enabler::Default);
            }
        }
        for (feature, enabler) in features {
            if !declared.declared.contains_key(&feature) {
                return Err(ProfileError::UnknownFeature {
                    profile: name,
                    feature,
                    by: by.map_or_else(|| "the target".to_owned(), |by| format!("profile {by}")),
                    known: declared.declared.keys().cloned().collect(),
                }
                .into());
            }
            changed |= node.features.entry(feature).or_default().insert(enabler);
        }
        if changed {
            self.changes = self.changes.saturating_add(1);
            self.expand(at);
        }
        Ok(())
    }

    /// The node for `name` in `source`, read if new, and whether it is.
    fn node(&mut self, source: &Source, name: &ProfileName) -> Result<(usize, bool)> {
        if let Some(&at) = self.index.get(name) {
            let met = self.nodes.get(at).map(|node| &node.source);
            return match met {
                Some(met) if met != source => {
                    let (first, second) = (met.to_string(), source.to_string());
                    Err(if met.twin(source) {
                        ProfileError::Diverged { first, second }
                    } else {
                        ProfileError::SameName { name: name.clone(), first, second }
                    }
                    .into())
                },
                Some(_) | None => Ok((at, false)),
            };
        }
        let profile = self.load.load(source, name)?;
        let at = self.nodes.len();
        self.nodes.push(Node {
            profile,
            name: name.clone(),
            source: source.clone(),
            features: BTreeMap::new(),
            required_by: BTreeSet::new(),
            configured: false,
            defaults: false,
            no_defaults: false,
            active: BTreeSet::new(),
        });
        self.index.insert(name.clone(), at);
        Ok((at, true))
    }

    /// Turns on everything the features of the node at `at` turn on, in it and in what it
    /// requires, until nothing more is.
    fn expand(&mut self, at: usize) {
        let Some(node) = self.nodes.get_mut(at) else { return };
        let manifest = L::manifest(&node.profile);
        let this = node.name.clone();
        let mut asks: Vec<Ask> = Vec::new();
        for (dep, requirement) in &manifest.requires {
            if !requirement.optional && node.active.insert(dep.clone()) {
                asks.push(Ask { dep: dep.clone(), turn: None, weak: false });
            }
        }
        loop {
            let mut changed = false;
            let on: Vec<FeatureName> = node.features.keys().cloned().collect();
            for feature in on {
                let by = Enabler::Feature { profile: this.clone(), feature: feature.clone() };
                for each in manifest.features.declared.get(&feature).into_iter().flatten() {
                    match each {
                        FeatureRef::Feature(other) => {
                            changed |=
                                node.features.entry(other.clone()).or_default().insert(by.clone());
                        },
                        FeatureRef::Dep(dep) => {
                            if node.active.insert(dep.clone()) {
                                changed = true;
                                asks.push(Ask { dep: dep.clone(), turn: None, weak: false });
                            }
                        },
                        FeatureRef::Of { profile: dep, feature: theirs, weak } => {
                            if !weak && node.active.insert(dep.clone()) {
                                changed = true;
                                asks.push(Ask { dep: dep.clone(), turn: None, weak: false });
                            }
                            // A weak feature reaches a profile active by any other means.
                            let own = node.active.contains(dep);
                            if own || self.index.contains_key(dep) {
                                let turn = Some((theirs.clone(), by.clone()));
                                asks.push(Ask { dep: dep.clone(), turn, weak: !own });
                            }
                        },
                    }
                }
            }
            if !changed {
                break;
            }
        }
        asks.sort();
        asks.dedup();
        for Ask { dep, turn, weak } in asks {
            let Some(requirement) = manifest.requires.get(&dep) else { continue };
            let by = Some(this.clone());
            if weak {
                let features = turn.into_iter().collect();
                self.queue.push_back(Request {
                    source: None,
                    name: dep,
                    by,
                    features,
                    defaults: false,
                });
                continue;
            }
            let source = requirement.source.clone().unwrap_or_else(|| node.source.clone());
            let mut features: Vec<Turn> = requirement
                .features
                .iter()
                .map(|f| (f.clone(), Enabler::Requires(this.clone())))
                .collect();
            features.extend(turn);
            let defaults = requirement.default_features;
            self.queue.push_back(Request {
                source: Some(source),
                name: dep,
                by,
                features,
                defaults,
            });
        }
    }

    /// Every node's position in the order the profiles apply: depth first from each configured
    /// layer, each requirement, by name, before its requirer.
    fn order(&self, configured: &[Configured<'_>]) -> Result<Vec<usize>> {
        let mut order = Vec::with_capacity(self.nodes.len());
        let mut done = vec![false; self.nodes.len()];
        let mut path = Vec::new();
        for layer in configured {
            if let Some(&at) = self.index.get(layer.name) {
                self.visit(at, &mut done, &mut path, &mut order)?;
            }
        }
        Ok(order)
    }

    /// Visits the node at `at` for [`Builder::order`]; `path` holds the nodes being visited.
    fn visit(
        &self, at: usize, done: &mut [bool], path: &mut Vec<usize>, order: &mut Vec<usize>,
    ) -> Result<()> {
        if done.get(at).copied().unwrap_or(true) {
            return Ok(());
        }
        let name = |at: usize| self.nodes.get(at).map(|node| node.name.clone());
        if let Some(from) = path.iter().position(|&on| on == at) {
            let chain = path
                .get(from..)
                .unwrap_or_default()
                .iter()
                .chain([&at])
                .filter_map(|&at| name(at))
                .collect();
            return Err(ProfileError::Cycle { chain }.into());
        }
        path.push(at);
        let active = self.nodes.get(at).map(|node| node.active.iter()).into_iter().flatten();
        for dep in active {
            if let Some(&next) = self.index.get(dep) {
                self.visit(next, done, path, order)?;
            }
        }
        path.pop();
        if let Some(visited) = done.get_mut(at) {
            *visited = true;
        }
        order.push(at);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeMap;
    use core::cell::RefCell;

    use super::{Configured, Enabler, Graph, Load, Node, Warning};
    use crate::errors::{Error, ProfileError, Result};
    use crate::name::{FeatureName, ProfileName};
    use crate::profile::Manifest;
    use crate::source::Source;

    /// Manifests by name, one source; every load recorded.
    struct Memory {
        manifests: BTreeMap<String, Manifest>,
        loaded: RefCell<Vec<String>>,
    }

    impl Load for Memory {
        type Profile = Manifest;

        fn load(&mut self, _: &Source, name: &ProfileName) -> Result<Manifest> {
            self.loaded.borrow_mut().push(name.to_string());
            Ok(self.manifests.get(name.as_str()).cloned().expect("a profile the test declares"))
        }

        fn manifest(profile: &Manifest) -> &Manifest {
            profile
        }
    }

    /// A source of `profiles`, each `(name, the rest of its profile.toml)`.
    fn memory(profiles: &[(&str, &str)]) -> Memory {
        let manifests = profiles
            .iter()
            .map(|(name, rest)| {
                let toml = format!("[profile]\nname = \"{name}\"\n{rest}");
                let manifest: Manifest = toml::from_str(&toml).expect("a manifest");
                manifest.check(name).expect("a valid manifest");
                ((*name).to_owned(), manifest)
            })
            .collect();
        Memory { manifests, loaded: RefCell::new(Vec::new()) }
    }

    /// The graph of `layers`, each `(name, features, defaults)`.
    fn build(memory: &mut Memory, layers: &[(&str, &[&str], bool)]) -> Result<Graph<Manifest>> {
        let source = Source::Dir("p".into());
        let names: Vec<(ProfileName, Vec<FeatureName>, bool)> = layers
            .iter()
            .map(|(name, features, defaults)| {
                let features = features.iter().map(|f| f.parse().expect("a feature")).collect();
                (name.parse().expect("a name"), features, *defaults)
            })
            .collect();
        let configured: Vec<Configured<'_>> = names
            .iter()
            .map(|(name, features, defaults)| Configured {
                source: &source,
                name,
                features,
                defaults: *defaults,
            })
            .collect();
        Graph::build(&configured, memory)
    }

    /// Each node's name and its features, with who turned each on.
    fn features(graph: &Graph<Manifest>) -> Vec<(String, Vec<String>)> {
        graph
            .nodes
            .iter()
            .map(|node: &Node<Manifest>| {
                let on = node.features.iter().map(|(feature, by)| {
                    let by: Vec<String> = by.iter().map(Enabler::to_string).collect();
                    format!("{feature} <- {}", by.join(", "))
                });
                (node.name.to_string(), on.collect())
            })
            .collect()
    }

    #[test]
    fn defaults_and_implied_features() {
        let mut memory = memory(&[(
            "book",
            "[features]\ndefault = [\"katex\"]\nkatex = []\napi = [\"katex\"]\nmermaid = []\n",
        )]);
        let graph = build(&mut memory, &[("book", &["api"], true)]).expect("a graph");
        assert_eq!(
            features(&graph),
            [(
                "book".to_owned(),
                vec!["api <- target".to_owned(), "katex <- default, book/api".to_owned()]
            )],
            "the defaults, and what a feature implies"
        );
        let graph = build(&mut memory, &[("book", &[], false)]).expect("a graph");
        assert_eq!(
            features(&graph),
            [("book".to_owned(), vec![])],
            "no defaults when switched off"
        );
    }

    #[test]
    fn features_of_requirements_unify() {
        let mut memory = memory(&[
            ("ci", "[features]\npages = []\nnightly = []\n"),
            ("book", "[requires]\nci = { optional = true }\n[features]\npages = [\"ci/pages\"]\n"),
            (
                "rust",
                "[requires]\nci = { features = [\"nightly\"] }\nbook = {}\n[features]\ndefault = [\"docs\"]\ndocs = [\"book/pages\"]\n",
            ),
        ]);
        let graph = build(&mut memory, &[("rust", &[], true)]).expect("a graph");
        assert_eq!(
            features(&graph),
            [
                (
                    "ci".to_owned(),
                    vec!["nightly <- rust".to_owned(), "pages <- book/pages".to_owned()]
                ),
                ("book".to_owned(), vec!["pages <- rust/docs".to_owned()]),
                ("rust".to_owned(), vec!["docs <- default".to_owned()]),
            ],
            "one node per profile, the union of what each asks, requirements first"
        );
        let ci = graph.nodes.iter().find(|n| n.name.as_str() == "ci").expect("ci");
        assert_eq!(ci.required_by.len(), 2, "both requirers");
    }

    #[test]
    fn optional_and_weak_requirements() {
        let mut memory = memory(&[
            ("lychee", "[features]\nstrict = []\n"),
            ("hook", "[features]\nstrict = []\n"),
            (
                "book",
                "[requires]\nlychee = { optional = true }\nhook = { optional = true }\n[features]\nlinks = [\"dep:lychee\"]\nstrict = [\"lychee?/strict\", \"hook?/strict\"]\n",
            ),
        ]);
        let graph = build(&mut memory, &[("book", &["strict"], true)]).expect("a graph");
        assert_eq!(
            features(&graph),
            [("book".to_owned(), vec!["strict <- target".to_owned()])],
            "a weak feature activates nothing"
        );
        assert_eq!(
            *memory.loaded.borrow(),
            ["book"],
            "an optional requirement nobody activates is never read"
        );
        let graph = build(&mut memory, &[("book", &["strict", "links"], true)]).expect("a graph");
        let names: Vec<String> = graph.nodes.iter().map(|n| n.name.to_string()).collect();
        assert_eq!(
            names,
            ["lychee", "book"],
            "`dep:` activates it, and the weak feature then reaches it"
        );
        assert_eq!(features(&graph)[0].1, ["strict <- book/strict"], "through the weak feature");
    }

    #[test]
    fn a_weak_feature_reaches_a_profile_active_by_other_means() {
        let mut memory = memory(&[
            ("lychee", "[features]\nstrict = []\n"),
            (
                "book",
                "[requires]\nlychee = { optional = true }\n[features]\nagents = [\"lychee?/strict\"]\n",
            ),
        ]);
        let graph = build(&mut memory, &[("book", &["agents"], true)]).expect("a graph");
        assert_eq!(graph.nodes.len(), 1, "alone, it activates nothing");
        let graph = build(&mut memory, &[("book", &["agents"], true), ("lychee", &[], true)])
            .expect("a graph");
        let lychee = graph.nodes.iter().find(|n| n.name.as_str() == "lychee").expect("lychee");
        let on: Vec<String> = lychee.features.keys().map(ToString::to_string).collect();
        assert_eq!(on, ["strict"], "the target's lychee takes it, whatever the order");
        assert!(lychee.required_by.is_empty(), "and is no requirement of book");
    }

    #[test]
    fn defaults_a_requirer_turns_on_are_named() {
        let mut memory = memory(&[
            ("book", "[features]\ndefault = [\"katex\"]\nkatex = []\n"),
            ("rust", "[requires]\nbook = {}\n"),
        ]);
        let graph =
            build(&mut memory, &[("book", &[], false), ("rust", &[], true)]).expect("a graph");
        assert_eq!(features(&graph)[0].1, ["katex <- default"], "on, since a requirer asks");
        let warning = Warning::DefaultsOn {
            profile: "book".parse().expect("a name"),
            by: "rust".parse().expect("a name"),
        };
        assert_eq!(graph.warnings, [warning], "and the target hears who");
    }

    #[test]
    fn what_cannot_hold_is_refused() {
        let mut memory = memory(&[
            ("a", "[requires]\nb = {}\n"),
            ("b", "[requires]\na = {}\n"),
            ("c", "[requires]\nd = { features = [\"nope\"] }\n"),
            ("d", "[features]\nyes = []\n"),
        ]);
        let cycle = build(&mut memory, &[("a", &[], true)]).expect_err("a cycle");
        assert!(
            matches!(cycle, Error::Profile(ProfileError::Cycle { ref chain }) if chain.len() == 3),
            "{cycle}"
        );
        let unknown = build(&mut memory, &[("c", &[], true)]).expect_err("an unknown feature");
        assert!(
            matches!(unknown, Error::Profile(ProfileError::UnknownFeature { ref by, .. }) if by == "profile c"),
            "{unknown}"
        );
        let unknown = build(&mut memory, &[("d", &["no"], true)]).expect_err("an unknown feature");
        assert!(
            matches!(unknown, Error::Profile(ProfileError::UnknownFeature { ref by, .. }) if by == "the target"),
            "{unknown}"
        );
    }
}
