//! Dogfood harness for the synchronizer release-train library.
//!
//! This crate consumes the synchronizer *library* (pinned by git revision)
//! and drives its real chain — `ReleaseTrainRun::execute` → discovery →
//! attestation → `resolve_closure` → `write_integration_artifacts` — against
//! the real six-crate language-family stack on GitHub. It exists to produce
//! the first non-synthetic release-train closure artifacts and to record,
//! from a genuine caller's seat, exactly where the documented CLI/skill would
//! strand a user versus what the library actually requires.
//!
//! The harness never edits the synchronizer repository; it is a downstream
//! consumer only.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::Command;

use synchronizer::build_verify::VerifyPolicy;
use synchronizer::component_manifests::ComponentManifests;
use synchronizer::configuration::{
    BranchScheme, BuilderResolution, ClusterSource, CommitAuthor, Component, ComponentCheckout,
    Forge, ForgeOwner, SynchronizerConfig,
};
use synchronizer::git_repository::{ComponentRepository, GitRepository, RepositoryFilePath};
use synchronizer::release_train::{
    CandidateSelector, ComponentLockIdentity, NixSourceAttestation, ReleaseTrainIntent,
    ReleaseTrainRun, ResolvedComponent, ResolvedSelector, TrainComponent,
};
use synchronizer::topology::DependencyGraph;
use synchronizer::types::{
    AbsolutePath, AuthorEmail, AuthorName, BranchName, BuilderRole, CommitIdentifier,
    ComponentName, NarHash, RepositoryUrl,
};

/// Typed failures at this harness's boundary. Every underlying synchronizer or
/// release-train error is carried as its rendered detail — this crate is a
/// driver, not a place to re-model the library's error algebra.
#[derive(Debug, thiserror::Error)]
pub enum DogfoodError {
    #[error("release-train intent decode failed: {0}")]
    IntentDecode(String),
    #[error("reading {path}: {detail}")]
    Io { path: String, detail: String },
    #[error("git {operation} failed for {target}: {detail}")]
    GitCommand {
        operation: String,
        target: String,
        detail: String,
    },
    #[error("release-train library error: {0}")]
    ReleaseTrain(String),
    #[error("synchronizer library error: {0}")]
    Synchronizer(String),
    #[error("nix flake prefetch of {reference} failed: {detail}")]
    Prefetch { reference: String, detail: String },
    #[error("nix flake prefetch output for {reference} carried no hash field: {output}")]
    PrefetchShape { reference: String, output: String },
}

/// The whole harness: where the component clones live, which forge owner holds
/// them, the decoded train intent, and the commit author stamped on candidate
/// commits.
pub struct DogfoodHarness {
    checkout_root: PathBuf,
    owner: String,
    author: CommitAuthor,
    intent: ReleaseTrainIntent,
}

impl DogfoodHarness {
    /// Decode the authored intent NOTA and bind it to a checkout root and
    /// forge owner.
    pub fn from_intent_text(
        checkout_root: PathBuf,
        owner: impl Into<String>,
        intent_text: &str,
    ) -> Result<Self, DogfoodError> {
        let intent = ReleaseTrainIntent::from_nota_text(intent_text)
            .map_err(|error| DogfoodError::IntentDecode(error.to_string()))?;
        Ok(Self {
            checkout_root,
            owner: owner.into(),
            author: CommitAuthor::new(
                AuthorName::new("release-train-dogfood"),
                AuthorEmail::new("ligoldragon@gmail.com"),
            ),
            intent,
        })
    }

    pub fn intent(&self) -> &ReleaseTrainIntent {
        &self.intent
    }

    /// The declared members, in authored order.
    pub fn train_components(&self) -> &[TrainComponent] {
        self.intent.components()
    }

    pub fn component_names(&self) -> Vec<ComponentName> {
        self.intent
            .components()
            .iter()
            .map(|component| component.component().clone())
            .collect()
    }

    /// The operational configuration the release-train run reads. The staging
    /// branch here is a placeholder; the library's `release_train_view`
    /// overrides it with the `train/<name>` candidate branch for the run.
    ///
    /// Builder resolution is deliberately pointed at an absent cluster proposal
    /// so host resolution fails fast: the release-train run then records every
    /// per-component verification as `NotAttempted` and performs no ssh, no
    /// `nix build`. Nix-level proof of the closure is produced separately,
    /// against the generated integration flake.
    pub fn configuration(&self) -> SynchronizerConfig {
        let components = self
            .component_names()
            .into_iter()
            .map(|name| Component::new(name, ComponentCheckout::AtRoot))
            .collect();
        SynchronizerConfig::new(
            Forge::GitHub(ForgeOwner::new(self.owner.clone())),
            AbsolutePath::new(self.checkout_root.to_string_lossy().to_string()),
            components,
            self.branch_scheme(),
            BuilderResolution::ClusterRole(
                BuilderRole::new("NixBuilder"),
                ClusterSource::ClusterProposal(AbsolutePath::new(
                    "/nonexistent/release-train-dogfood-no-cluster.nota",
                )),
            ),
            VerifyPolicy::DefaultBuild,
            self.author.clone(),
        )
    }

    fn branch_scheme(&self) -> BranchScheme {
        BranchScheme::new(BranchName::new("main"), BranchName::new("synchronizer"))
    }

    fn repository_url(&self, name: &ComponentName) -> RepositoryUrl {
        RepositoryUrl::new(format!(
            "https://github.com/{}/{}.git",
            self.owner,
            name.as_str()
        ))
    }

    fn checkout_path(&self, name: &ComponentName) -> PathBuf {
        self.checkout_root.join(name.as_str())
    }

    /// Clone every declared component into the checkout root if it is not
    /// already present. The release-train run opens these as git object stores
    /// and push origins; it never touches their working copies.
    pub fn clone_missing(&self) -> Result<Vec<ComponentName>, DogfoodError> {
        let mut cloned = Vec::new();
        for name in self.component_names() {
            let path = self.checkout_path(&name);
            if path.join(".git").is_dir() {
                continue;
            }
            let url = self.repository_url(&name);
            let output = Command::new("git")
                .args(["clone", "--quiet", url.as_str()])
                .arg(&path)
                .output()
                .map_err(|error| DogfoodError::GitCommand {
                    operation: "clone".to_string(),
                    target: name.as_str().to_string(),
                    detail: error.to_string(),
                })?;
            if !output.status.success() {
                return Err(DogfoodError::GitCommand {
                    operation: "clone".to_string(),
                    target: name.as_str().to_string(),
                    detail: String::from_utf8_lossy(&output.stderr).to_string(),
                });
            }
            cloned.push(name);
        }
        Ok(cloned)
    }

    /// Open one component's production git surface at its configured clone.
    pub fn open_repository(&self, name: &ComponentName) -> Result<GitRepository, DogfoodError> {
        GitRepository::open(
            name.clone(),
            self.checkout_path(name),
            self.repository_url(name),
            self.branch_scheme(),
            self.author.clone(),
        )
        .map_err(|error| DogfoodError::Synchronizer(error.to_string()))
    }

    /// Run the library's real dependency discovery over the manifests at the
    /// given revisions, and return the graph together with the internal
    /// component set the manifests actually carry.
    ///
    /// This is where defect 2's exact gap lives: `DependencyGraph::discover`
    /// runs and produces a real graph, but the graph exposes no accessor for
    /// its component set, and discovery never records external (non-configured)
    /// dependency commits at all — so neither `resolve_closure` argument
    /// (`discovered_internal_components`, `discovered_external_components`) can
    /// be obtained from the discovery result. The caller must hand-assemble
    /// both from the manifest list, which is what `internal` returns here.
    pub fn discover_topology(
        &self,
        revisions: &[(ComponentName, CommitIdentifier)],
    ) -> Result<DiscoveredTopology, DogfoodError> {
        let configuration = self.configuration();
        let mut manifests = Vec::new();
        for (name, revision) in revisions {
            let repository = self.open_repository(name)?;
            repository
                .fetch(revision)
                .map_err(|error| DogfoodError::Synchronizer(error.to_string()))?;
            let manifest = ComponentManifests::load_at(&repository, name, revision.clone())
                .map_err(|error| DogfoodError::Synchronizer(error.to_string()))?;
            manifests.push(manifest);
        }
        let graph = DependencyGraph::discover(&configuration, &manifests)
            .map_err(|error| DogfoodError::Synchronizer(error.to_string()))?;
        let internal = manifests
            .iter()
            .map(|manifest| manifest.component().clone())
            .collect();
        Ok(DiscoveredTopology { graph, internal })
    }

    /// The SRI narHash Nix computes for a component's tree at `revision`,
    /// obtained through the same `nix flake prefetch` boundary the library's
    /// flake-lock bump uses. The attestation the closure needs binds this hash
    /// to the *candidate* commit (the train-branch tip), not the source
    /// selector.
    pub fn narhash_of(
        &self,
        component: &ComponentName,
        revision: &CommitIdentifier,
    ) -> Result<NarHash, DogfoodError> {
        let reference = format!(
            "github:{}/{}/{}",
            self.owner,
            component.as_str(),
            revision.as_str()
        );
        let output = Command::new("nix")
            .args(["flake", "prefetch", "--json", "--refresh", &reference])
            .output()
            .map_err(|error| DogfoodError::Prefetch {
                reference: reference.clone(),
                detail: error.to_string(),
            })?;
        if !output.status.success() {
            return Err(DogfoodError::Prefetch {
                reference,
                detail: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }
        let text = String::from_utf8_lossy(&output.stdout).to_string();
        Self::json_string_field(&text, "hash")
            .map(NarHash::new)
            .ok_or(DogfoodError::PrefetchShape {
                reference,
                output: text,
            })
    }

    /// Extract a JSON string field's value by key from flat object text,
    /// without a JSON dependency. The prefetch output is a single controlled
    /// object; this finds `"<key>":"` and reads to the next quote.
    fn json_string_field(text: &str, key: &str) -> Option<String> {
        let needle = format!("\"{key}\":\"");
        let start = text.find(&needle)? + needle.len();
        let rest = &text[start..];
        let end = rest.find('"')?;
        Some(rest[..end].to_string())
    }

    /// Read the Cargo and flake lock texts a component carries at `revision`,
    /// for the per-component lock identity in the closure. A missing layer
    /// contributes empty text — the identity still records that the layer was
    /// empty at that revision.
    pub fn lock_texts(
        &self,
        repository: &GitRepository,
        revision: &CommitIdentifier,
    ) -> Result<(String, String), DogfoodError> {
        let cargo_lock = repository
            .file_at(revision, &RepositoryFilePath::cargo_lock())
            .map_err(|error| DogfoodError::Synchronizer(error.to_string()))?
            .unwrap_or_default();
        let flake_lock = repository
            .file_at(revision, &RepositoryFilePath::flake_lock())
            .map_err(|error| DogfoodError::Synchronizer(error.to_string()))?
            .unwrap_or_default();
        Ok((cargo_lock, flake_lock))
    }

    /// Compute the genuine attestations and per-component lock identities for
    /// the resolved selectors: a real narHash per candidate commit and a
    /// blake3 over each candidate's real Cargo/flake lock text.
    pub fn attest_selectors(
        &self,
        selectors: &[ResolvedSelector],
    ) -> Result<Attestations, DogfoodError> {
        let mut nix_sources = Vec::new();
        let mut locks = Vec::new();
        for selector in selectors {
            let component = selector.component();
            let candidate = selector.candidate();
            let nar_hash = self.narhash_of(component, candidate)?;
            nix_sources.push(NixSourceAttestation::new(
                component.clone(),
                candidate.clone(),
                nar_hash,
            ));
            let repository = self.open_repository(component)?;
            repository
                .fetch(candidate)
                .map_err(|error| DogfoodError::Synchronizer(error.to_string()))?;
            let (cargo_lock, flake_lock) = self.lock_texts(&repository, candidate)?;
            locks.push(ComponentLockIdentity::from_text(
                component.clone(),
                &cargo_lock,
                &flake_lock,
            ));
        }
        Ok(Attestations { nix_sources, locks })
    }
}

/// The real discovered topology and the hand-assembled internal component set
/// that `resolve_closure` requires.
pub struct DiscoveredTopology {
    pub graph: DependencyGraph,
    pub internal: BTreeSet<ComponentName>,
}

/// Genuine closure inputs computed by the harness because the run does not
/// capture them itself.
pub struct Attestations {
    pub nix_sources: Vec<NixSourceAttestation>,
    pub locks: Vec<ComponentLockIdentity>,
}

/// One end-to-end dogfood invocation: which intent to drive, where the
/// component clones live, where to write the portable artifacts.
pub struct DogfoodInvocation {
    intent_path: PathBuf,
    checkout_root: PathBuf,
    output_directory: PathBuf,
    owner: String,
}

impl DogfoodInvocation {
    pub fn new(
        intent_path: PathBuf,
        checkout_root: PathBuf,
        output_directory: PathBuf,
        owner: impl Into<String>,
    ) -> Self {
        Self {
            intent_path,
            checkout_root,
            output_directory,
            owner: owner.into(),
        }
    }

    /// Drive the whole library chain, printing the stage-by-stage evidence
    /// each gating defect needs as its acceptance fixture.
    pub fn run(&self) -> Result<(), DogfoodError> {
        let intent_text =
            std::fs::read_to_string(&self.intent_path).map_err(|error| DogfoodError::Io {
                path: self.intent_path.display().to_string(),
                detail: error.to_string(),
            })?;
        let harness = DogfoodHarness::from_intent_text(
            self.checkout_root.clone(),
            self.owner.clone(),
            &intent_text,
        )?;

        println!("== STAGE a: intent decoded (typed) ==");
        println!("intent file: {}", self.intent_path.display());
        println!("train name:  {}", harness.intent().name().as_str());
        println!(
            "candidate branch: train/{}",
            harness.intent().name().as_str()
        );
        for component in harness.train_components() {
            let selector = match component.selector() {
                CandidateSelector::Mainline => "Mainline".to_string(),
                CandidateSelector::Branch(branch) => format!("Branch({})", branch.as_str()),
                CandidateSelector::ExactCommit(commit) => {
                    format!("ExactCommit({})", commit.as_str())
                }
            };
            println!(
                "  {:<24} selector={:<20} expected_base={}",
                component.component().as_str(),
                selector,
                component.expected_base().as_str()
            );
        }
        println!(
            "immutable externals declared: {}",
            harness.intent().immutable_externals().len()
        );

        println!("\n== cloning components (object stores + push origins) ==");
        let cloned = harness.clone_missing()?;
        if cloned.is_empty() {
            println!("all component clones already present");
        } else {
            for name in &cloned {
                println!("  cloned {}", name.as_str());
            }
        }

        println!("\n== STAGE b: ReleaseTrainRun::execute() against real remotes ==");
        let configuration = harness.configuration();
        let materialized = ReleaseTrainRun::from_config(configuration, harness.intent().clone())
            .execute()
            .map_err(|error| DogfoodError::ReleaseTrain(error.to_string()))?;
        println!(
            "candidate branch materialized: {}",
            materialized.candidate_branch().as_str()
        );
        println!("resolved selectors (pushed train/<name> tips):");
        for selector in materialized.selectors() {
            println!(
                "  {:<24} selected={} candidate(train tip)={}",
                selector.component().as_str(),
                selector.selected().as_str(),
                selector.candidate().as_str()
            );
        }
        println!(
            "\ncascade report (has_failures={}):",
            materialized.report().has_failures()
        );
        match materialized.report().to_nota_text() {
            Ok(text) => println!("{text}"),
            Err(error) => println!("(report NOTA rendering failed: {error})"),
        }

        let revisions: Vec<_> = materialized
            .selectors()
            .iter()
            .map(|selector| (selector.component().clone(), selector.candidate().clone()))
            .collect();

        println!("== STAGE c: real dependency discovery (DependencyGraph::discover) ==");
        let topology = harness.discover_topology(&revisions)?;
        println!(
            "discovered internal components: {}",
            topology.internal.len()
        );
        for name in &topology.internal {
            println!("  internal: {}", name.as_str());
        }
        println!("discovered dependency edges (consumer -> producer @ layer):");
        for edge in topology.graph.edges() {
            println!(
                "  {} -> {} [{:?} / {:?}]",
                edge.consumer().as_str(),
                edge.producer().as_str(),
                edge.layer(),
                edge.local_name()
            );
        }
        match topology.graph.ascent_levels() {
            Ok(levels) => {
                for (index, level) in levels.levels().iter().enumerate() {
                    let names: Vec<&str> = level.iter().map(|name| name.as_str()).collect();
                    println!("  ascent level {index}: {}", names.join(", "));
                }
            }
            Err(error) => println!("  ascent_levels failed: {error}"),
        }

        println!("\n== STAGE d: real attestations (narHash) + per-component lock identities ==");
        let attestations = harness.attest_selectors(materialized.selectors())?;
        for source in &attestations.nix_sources {
            println!(
                "  {:<24} candidate={} narHash={}",
                source.component().as_str(),
                source.commit().as_str(),
                source.nar_hash().as_str()
            );
        }

        println!("\n== STAGE e: resolve_closure -> ResolvedReleaseTrain ==");
        let external = BTreeMap::new();
        let closure = materialized
            .resolve_closure(
                attestations.nix_sources,
                attestations.locks,
                topology.internal.clone(),
                external,
            )
            .map_err(|error| DogfoodError::ReleaseTrain(error.to_string()))?;
        println!("closure identity: {}", closure.identity());
        println!("candidate branch: {}", closure.candidate_branch().as_str());
        for component in closure.components() {
            println!(
                "  resolved component: {}",
                Self::resolved_component_line(component)
            );
        }

        println!("\n== STAGE f: write_integration_artifacts ==");
        let artifacts = closure
            .write_integration_artifacts(&self.output_directory, &self.owner)
            .map_err(|error| DogfoodError::ReleaseTrain(error.to_string()))?;
        println!(
            "release-train.lock.json: {}",
            artifacts.json_path().display()
        );
        println!(
            "integration flake.nix:   {}",
            artifacts.flake_path().display()
        );
        println!("\n-- release-train.lock.json --");
        Self::print_file(artifacts.json_path());
        println!("\n-- integration flake.nix --");
        Self::print_file(artifacts.flake_path());

        println!("\n== dogfood library chain complete (stages a-f) ==");
        Ok(())
    }

    fn resolved_component_line(component: &ResolvedComponent) -> String {
        format!("{component:?}")
    }

    fn print_file(path: &std::path::Path) {
        match std::fs::read_to_string(path) {
            Ok(text) => println!("{text}"),
            Err(error) => println!("(unreadable {}: {error})", path.display()),
        }
    }
}
