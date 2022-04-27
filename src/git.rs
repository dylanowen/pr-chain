use anyhow::{anyhow, Context};
use git2::{Branch, BranchType, FetchOptions, Oid, RemoteCallbacks, Repository, Revwalk};
use git2_credentials::ui4dialoguer::CredentialUI4Dialoguer;
use git2_credentials::CredentialHandler;
use tokio::process::Command;

pub fn get_branch<'repo>(
    name: &str,
    repo: &'repo Repository,
) -> anyhow::Result<(Branch<'repo>, Oid)> {
    let branch = repo
        .find_branch(name, BranchType::Local)
        .with_context(|| format!("Couldn't find branch '{name}'"))?;
    let id = branch
        .get()
        .target()
        .ok_or_else(|| anyhow!("Couldn't find a reference for {name}"))?;

    Ok((branch, id))
}

pub fn fetch_remotes(repo: &Repository) -> anyhow::Result<()> {
    for remote_name in repo
        .remotes()
        .context("Couldn't get remotes")?
        .into_iter()
        .flatten()
    {
        log::info!("Fetching remote: {remote_name}");
        repo.find_remote(remote_name)
            .with_context(|| format!("Couldn't get remote '{remote_name}'"))?
            .fetch::<&str>(&[], Some(&mut default_fetch_options()?), None)
            .with_context(|| format!("Couldn't fetch remote '{remote_name}'"))?;
    }

    Ok(())
}

pub fn default_fetch_options() -> anyhow::Result<FetchOptions<'static>> {
    let mut callbacks = RemoteCallbacks::new();

    let git_config = git2::Config::open_default()?;
    let mut ch = CredentialHandler::new_with_ui(git_config, Box::new(CredentialUI4Dialoguer {}));

    callbacks
        .credentials(move |url, username, allowed| ch.try_next_credential(url, username, allowed));
    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(callbacks);

    Ok(fetch_options)
}

pub async fn log(revisions: &[&str]) -> anyhow::Result<String> {
    let output_result = Command::new("git")
        .arg("log")
        .arg("--graph")
        .arg("--full-history")
        .arg("--all")
        .arg("--date-order")
        .arg("--color")
        .arg("--pretty=format:\"%x1b[31m%h%x09%x1b[32m%d%x1b[0m%x20%s\"")
        .args(revisions)
        .output()
        .await?;

    Ok(String::from_utf8_lossy(&output_result.stdout).to_string())
}

pub fn collect_revwalk(walk: &mut Revwalk) -> anyhow::Result<Vec<Oid>> {
    let mut ids = Vec::new();
    for id in walk.by_ref() {
        ids.push(id?);
    }
    ids.reverse();

    Ok(ids)
}
