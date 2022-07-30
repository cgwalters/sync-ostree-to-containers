use std::collections::HashSet;
use std::process::Command;

use anyhow::Result;
use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use once_cell::sync::Lazy;

/// Arbitrary list; this is all that's shipped in the ostree repo
/// for Fedora today.
#[allow(dead_code)]
static ARCHITECTURES: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    ["aarch64", "x86_64", "ppc64le", "s390x"]
        .into_iter()
        .collect()
});

#[derive(Debug, Parser)]
struct Opt {
    /// Path to OSTree repository
    #[clap(long, value_parser, global = true)]
    repo: Utf8PathBuf,

    /// Ostree remote name
    #[clap(long, value_parser, global = true)]
    remote: String,

    /// The ostree container format version
    #[clap(long, default_value = "1")]
    format_version: u32,

    #[clap(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, clap::Subcommand)]
enum Cmd {
    /// Fetch multiple ostree refs
    Fetch {
        /// A refspec that supports globs; for example,
        /// fedora/36/*/updates
        refs: String,
    },
}

impl Opt {
    fn run(self) -> Result<()> {
        match &self.cmd {
            Cmd::Fetch { refs } => self.fetch(refs),
        }
    }

    fn fetch(&self, refglob: &str) -> Result<()> {
        let all_refs = remote_list(self.repo.as_path(), &self.remote)?;
        let all_refs = all_refs.iter().map(|s| s.as_str()).collect::<Vec<_>>();

        let targets = glob_match_refs(&all_refs, refglob);

        println!("{targets:?}");

        Ok(())
    }
}

fn remote_list(repo: &Utf8Path, remote: &str) -> Result<Vec<String>> {
    let o = Command::new("ostree")
        .args(["--repo", repo.as_str(), "remote", "list", remote])
        .output()?;
    if !o.status.success() {
        anyhow::bail!("failed to run ostree remote list: {:?}", o.status)
    }
    let o = String::from_utf8(o.stdout)?;
    Ok(o.lines().map(|v| v.to_string()).collect())
}

fn glob_match_refs<'a>(all_refs: &'a [&str], glob: &str) -> Vec<&'a str> {
    let parts = glob.split('/').collect::<Vec<_>>();
    all_refs
        .iter()
        .filter(|v| {
            let v_parts = v.split('/').collect::<Vec<_>>();
            if parts.len() != v_parts.len() {
                return false;
            }

            for (&v, &g) in v_parts.iter().zip(parts.iter()) {
                if g != "*" && v != g {
                    return false;
                }
            }

            true
        })
        .copied()
        .collect()
}

fn main() -> anyhow::Result<()> {
    let opts = Opt::from_args();

    opts.run()

    // Take as input a set of refs for example given
    //
    // fedora:fedora/36/aarch64/silverblue
    // fedora:fedora/36/aarch64/testing/silverblue
    // fedora:fedora/36/aarch64/updates/silverblue
    // fedora:fedora/36/ppc64le/silverblue
    // fedora:fedora/36/ppc64le/testing/silverblue
    // fedora:fedora/36/ppc64le/updates/silverblue
    // fedora:fedora/36/x86_64/silverblue
    // fedora:fedora/36/x86_64/testing/silverblue
    // fedora:fedora/36/x86_64/updates/silverblue
    //
    // We want to generate 3 containers:
    // quay.io/fedora/silverblue:36
    // quay.io/fedora/silverblue:36-testing
    // quay.io/fedora/silverblue:36-updates
    //
    // That should be manifest listed.
    //
    // Fetch the ostree commits only, and inspect their versions.  Error out
    // by default if they are different?
    //
    // Check if there's an existing manifest list image, i.e. skopeo inspect
    // or use the container proxy.
    // If there are any missing missing manifest architecture entries,
    // *or* if the manifest list version is different than the commit version,
    // fetch the target ostree commit (entirely).
    // run rpm-ostree container-encapsulate on it to an oci dir, then copy to containers-storage
    //
    // podman manifest create quay.io/fedora/silverblue:36
    // for arch in arches; podman manifest annotate --annotation version=version; done
}
