//! Which plugin serves a request: the registry built from what discovery found.

mod entries;

pub use entries::{Candidate, Fit, Installed, Origin, Readiness, Unusable};

use crate::manifest::{PluginId, Provides};
use std::cmp::Reverse;

/// The installed plugins, in the order that decides which serves a request.
///
/// Among manifests with one id, those with an unusable program or a protocol newer than the
/// host's are set aside first; of the rest the higher protocol version wins, and at equal
/// versions the person's directory wins over the system's. Plugins with different ids are tried
/// in that same order, then by id, so which one serves a request never depends on the order the
/// disk listed the files in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registry<P: Provides> {
    installed: Vec<Installed<P>>,
    unusable: Vec<Unusable<P>>,
}

impl<P: Provides> Default for Registry<P> {
    fn default() -> Self {
        Registry {
            installed: Vec::new(),
            unusable: Vec::new(),
        }
    }
}

impl<P: Provides> Registry<P> {
    /// No plugins.
    pub fn empty() -> Registry<P> {
        Registry::default()
    }

    /// The registry for `candidates`, applying the precedence above. `supported` is the newest
    /// protocol version the host speaks.
    pub fn resolve(candidates: Vec<Candidate<P>>, supported: u32) -> Registry<P> {
        let mut usable = Vec::new();
        let mut unusable = Vec::new();
        for candidate in candidates {
            match usability(&candidate, supported) {
                Ok(()) => usable.push(Installed {
                    manifest: candidate.manifest,
                    origin: candidate.origin,
                }),
                Err(reason) => unusable.push(Unusable {
                    id: candidate.manifest.id,
                    origin: candidate.origin,
                    reason,
                }),
            }
        }
        usable.sort_by_key(rank);
        let mut installed: Vec<Installed<P>> = Vec::with_capacity(usable.len());
        for plugin in usable {
            if installed
                .iter()
                .all(|kept| kept.manifest.id != plugin.manifest.id)
            {
                installed.push(plugin);
            }
        }
        unusable.sort_by(|a, b| (&a.id, a.origin).cmp(&(&b.id, b.origin)));
        Registry {
            installed,
            unusable,
        }
    }

    /// The usable plugins, in serving order.
    pub fn installed(&self) -> &[Installed<P>] {
        &self.installed
    }

    /// The plugins that were found and cannot be used.
    pub fn unusable(&self) -> &[Unusable<P>] {
        &self.unusable
    }

    /// The plugin that serves a request for `capability`: of those whose entry for it `fit`
    /// calls [`Fit::Exact`] the first in serving order, else of those it calls [`Fit::Broad`]
    /// the first, else none.
    pub fn serving(
        &self,
        capability: P::Capability,
        fit: impl Fn(&P) -> Fit,
    ) -> Option<&Installed<P>> {
        let mut broad = None;
        for plugin in &self.installed {
            let fits = plugin
                .manifest
                .provision(capability)
                .map_or(Fit::Miss, &fit);
            match fits {
                Fit::Exact => return Some(plugin),
                Fit::Broad => broad = broad.or(Some(plugin)),
                Fit::Miss => {}
            }
        }
        broad
    }

    /// Whether the plugin `id` is set aside only because the file named `tool` is missing or not
    /// executable: the tool is what is absent, not the plugin, and installing the tool is what
    /// makes the plugin usable.
    pub fn tool_absent(&self, id: &str, tool: &str) -> bool {
        use crate::manifest::ManifestError::{FileMissing, NotExecutable};
        self.unusable.iter().any(|unusable| {
            unusable.id.as_str() == id
                && matches!(
                    &unusable.reason,
                    FileMissing { path } | NotExecutable { path }
                        if path.file_name().is_some_and(|name| name == tool)
                )
        })
    }
}

fn usability<P: Provides>(
    candidate: &Candidate<P>,
    supported: u32,
) -> Result<(), crate::manifest::ManifestError<P::Capability, P::Fault>> {
    let protocol = candidate.manifest.protocol;
    if protocol > supported {
        return Err(crate::manifest::ManifestError::ProtocolUnsupported {
            protocol,
            supported,
        });
    }
    match &candidate.readiness {
        Readiness::Ready => Ok(()),
        Readiness::Unready(reason) => Err(reason.clone()),
    }
}

/// The serving order: newer protocol first, then the person's before the system's, then by id.
fn rank<P>(plugin: &Installed<P>) -> (Reverse<u32>, Reverse<Origin>, PluginId) {
    (
        Reverse(plugin.manifest.protocol),
        Reverse(plugin.origin),
        plugin.manifest.id.clone(),
    )
}

#[cfg(test)]
mod tests;
