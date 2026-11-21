//! The index reader isolates malformed optional rows; writers deserialize the strict types.
use super::{Index, LeftOut, NativeBinary, NativeBinaryRefusal, Project, Version, check_binary};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

#[derive(Deserialize)]
pub(super) struct ReadIndex {
    #[serde(flatten)]
    index: Index,
    #[serde(default)]
    collections: Vec<Value>,
    #[serde(default)]
    projects: Vec<ReadProject>,
}

#[derive(Deserialize)]
struct ReadProject {
    #[serde(flatten)]
    project: Project,
    #[serde(default)]
    versions: Vec<ReadVersion>,
}

#[derive(Deserialize)]
struct ReadVersion {
    #[serde(flatten)]
    version: Version,
    #[serde(default = "empty_rows")]
    native_binaries: Value,
}

fn empty_rows() -> Value {
    Value::Array(Vec::new())
}

impl ReadIndex {
    pub(super) fn validated(self, local_files: bool) -> Index {
        let Self {
            mut index,
            projects,
            collections,
        } = self;
        let mut builds: BTreeMap<&str, &str> = index
            .releases
            .iter()
            .flatten()
            .map(|release| (release.tag.as_str(), release.commit.as_str()))
            .collect();
        if let Some(nightly) = &index.nightly_build {
            builds.insert("nightly", &nightly.commit);
        }
        for ReadProject {
            mut project,
            versions,
        } in projects
        {
            for version in versions {
                let version =
                    version.keep_binaries(&project, &builds, local_files, &mut index.left_out);
                project.versions.push(version);
            }
            index.projects.push(project);
        }
        for (position, row) in collections.into_iter().enumerate() {
            let id = row
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| format!("collections[{position}]"));
            match serde_json::from_value(row) {
                Ok(collection) => index.collections.push(collection),
                Err(error) => index.left_out.push(LeftOut {
                    id,
                    version: None,
                    reason: format!("invalid collection: {error}"),
                }),
            }
        }
        index.keep_valid(local_files);
        index
    }
}

impl ReadVersion {
    fn keep_binaries(
        self,
        project: &Project,
        builds: &BTreeMap<&str, &str>,
        local_files: bool,
        left_out: &mut Vec<LeftOut>,
    ) -> Version {
        let Self {
            mut version,
            native_binaries,
        } = self;
        let mut rejected = |reason| left_out.push(LeftOut::version(&project.id, &version, reason));
        let mut refused = Vec::new();
        let mut accepted = Vec::new();
        match native_binaries {
            Value::Array(rows) => {
                let tuple = |row: &Value| {
                    row.get("build")
                        .and_then(Value::as_str)
                        .zip(row.get("target").and_then(Value::as_str))
                        .map(|(build, target)| (build.to_owned(), target.to_owned()))
                };
                let mut counts = HashMap::new();
                for key in rows.iter().filter_map(tuple) {
                    *counts.entry(key).or_insert(0) += 1;
                }
                for (position, row) in rows.into_iter().enumerate() {
                    let key = tuple(&row);
                    let result = if key.as_ref().is_some_and(|key| counts[key] > 1) {
                        Err("duplicate native build and target".into())
                    } else {
                        serde_json::from_value::<NativeBinary>(row)
                            .map_err(|e| e.to_string())
                            .and_then(|binary| {
                                check_binary(project, &binary, local_files)?;
                                Ok(binary)
                            })
                    };
                    match (result, key) {
                        (Ok(binary), _) => accepted.push(binary),
                        (Err(error), Some((build, target))) => {
                            let reason = format!("native binary {build} / {target}: {error}");
                            rejected(reason.clone());
                            refused.push(NativeBinaryRefusal {
                                build,
                                target,
                                reason,
                            });
                        }
                        (Err(error), None) => {
                            rejected(format!("native_binaries[{position}]: {error}"))
                        }
                    }
                }
            }
            _ => rejected("native_binaries must be a list".into()),
        }
        for (release, verdict) in &mut version.verdicts {
            verdict.targets.retain(|target| {
                builds.get(release.as_str()).is_some_and(|build| {
                    accepted
                        .iter()
                        .any(|binary| binary.build == *build && binary.target == *target)
                })
            });
        }
        version.native_binaries = accepted;
        version.native_binary_refusals = refused;
        version
    }
}
