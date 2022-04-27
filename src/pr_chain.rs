use std::fmt::{Debug, Formatter};

use anyhow::anyhow;
use colored::Colorize;
use git2::{Branch, BranchType, Commit, Oid, Repository};

use crate::git;

pub struct PrChain<'repo> {
    chain: Vec<PrBranch<'repo>>,
}

pub struct PrBranch<'repo> {
    branch: Branch<'repo>,
    commit_ids: Vec<Oid>,
}

impl<'repo> PrChain<'repo> {
    pub fn init(
        repo: &'repo Repository,
        branch_name: &str,
        trunk_name: &str,
    ) -> anyhow::Result<PrChain<'repo>> {
        let (trunk, trunk_id) = git::get_branch(trunk_name, repo)?;
        let (branch, branch_id) = git::get_branch(branch_name, repo)?;

        let base = repo.merge_base(trunk_id, branch_id)?;

        let mut walk = repo.revwalk()?;
        walk.push(branch_id)?;
        walk.hide(trunk_id)?;

        let main_pr = PrBranch {
            branch,
            commit_ids: git::collect_revwalk(&mut walk)?,
        };

        // construct the Prs for our non main branches
        let mut chain = repo
            .branches(Some(BranchType::Local))?
            .into_iter()
            .filter_map(|branch| branch.ok())
            .filter(|(other_branch, _branch_type)| {
                other_branch.get() != trunk.get() && other_branch.get() != main_pr.branch.get()
            })
            .filter(|(other_branch, _)| {
                if let Some(other_id) = other_branch.get().target() {
                    // check to see if this branch merges with master in the same location
                    if let Ok(other_base) = repo.merge_base(trunk_id, other_id) {
                        return other_base == base;
                    }
                }

                false
            })
            .map(|(other_branch, _)| {
                walk.reset()?;
                walk.push(other_branch.get().target().ok_or_else(|| {
                    anyhow!("Couldn't find a reference for {:?}", other_branch.name())
                })?)?;
                walk.hide(trunk_id)?;

                let commits = git::collect_revwalk(&mut walk)?;

                Ok(PrBranch {
                    branch: other_branch,
                    commit_ids: commits,
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        chain.push(main_pr);

        // if we have more than 1 branch in our chain we need to organize and reduce our commits
        if chain.len() > 1 {
            // if we've done everything right, the pr with the most commits should be the last
            // branch in our chain
            chain.sort_by(|a, b| a.commit_ids.len().cmp(&b.commit_ids.len()));

            // loop through the chain removing commits that have already been covered by a previous
            // branch
            for i in 1..chain.len() {
                let previous_commits = chain[i - 1].commit_ids.clone();

                for pr in chain.iter_mut().skip(i) {
                    for previous_commit in previous_commits.iter() {
                        if let Some(commit) = pr.commit_ids.first() {
                            if commit != previous_commit {
                                break;
                            } else {
                                pr.commit_ids.remove(0);
                            }
                        }
                    }
                }
            }

            // make sure to remove any branches without commits
            chain = chain
                .into_iter()
                .filter(|pr| {
                    if !pr.commit_ids.is_empty() {
                        true
                    } else {
                        log::warn!("Branch '{}' doesn't have any unique commits", pr.name());
                        false
                    }
                })
                .collect();
        }

        Ok(PrChain { chain })
    }

    pub async fn log_plan(&self, repo: &'repo Repository) -> anyhow::Result<()> {
        let first_commit = self.chain.first().unwrap().commit_ids.first().unwrap();
        let last_commit = self.chain.last().unwrap().commit_ids.last().unwrap();

        log::info!(
            "Git Graph to Rebase:\n{}",
            git::log(&[&last_commit.to_string(), &format!("{}^!", first_commit)]).await?
        );

        log::info!("Planned Git Graph");
        for branch in self.chain.iter().rev() {
            let mut commits = branch.commits(repo)?.into_iter().rev();
            if let Some(commit) = commits.next() {
                println!("{}", log_commit(&commit, Some(&branch.name())));
            }
            for commit in commits {
                println!("{}", log_commit(&commit, None));
            }
        }

        Ok(())
    }

    // fn planned_chain(&self, repo: &'repo Repository) -> anyhow::Result<Vec<Commit<'repo>>> {
    //     self.chain
    //         .iter()
    //         .flat_map(|pr| match pr.commits(repo) {
    //             Ok(commits) => commits.into_iter().map(|item| Ok(item)).collect(),
    //             Err(error) => vec![Err(error)],
    //         })
    //         .collect()
    // }
}

impl<'repo> PrBranch<'repo> {
    fn name(&self) -> String {
        self.branch
            .name()
            .map(|name| name.unwrap_or("<unnamed>").to_string())
            .unwrap_or_else(|e| format!("<{e}>"))
    }

    fn commits<'r>(&self, repo: &'r Repository) -> anyhow::Result<Vec<Commit<'r>>> {
        Ok(self
            .commit_ids
            .iter()
            .map(|id| repo.find_commit(*id))
            .collect::<Result<Vec<_>, _>>()?)
    }
}

impl<'repo> Debug for PrBranch<'repo> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Pr({}, {:?})", self.name(), self.commit_ids)
    }
}

fn log_commit(commit: &Commit, branch: Option<&str>) -> String {
    format!(
        "* {}{} {}",
        &commit.id().to_string()[0..7].red(),
        branch
            .map(|name| format!(" ({})", name).green())
            .unwrap_or_default(),
        commit.summary().unwrap_or_default()
    )
}

#[cfg(test)]
mod test {
    use git2::{Commit, Signature, Tree};
    use tempfile::Builder;

    use super::*;

    #[tokio::test]
    async fn test_logging() {
        let repo = setup_test_repo();
        let chain = PrChain::init(&repo, "branch-c", "main").unwrap();

        chain.log_plan(&repo).await.unwrap();

        println!("Test Repo: {:#?}", repo.path())
    }

    fn setup_test_repo() -> Repository {
        let repo = test_repo();
        {
            let tree = repo
                .find_tree(repo.index().unwrap().write_tree().unwrap())
                .unwrap();

            let init = commit("HEAD", "init 1", &tree, &[], &repo);
            let init = commit("HEAD", "init 2", &tree, &[&init], &repo);
            let _main1 = commit("HEAD", "next_main", &tree, &[&init], &repo);

            let _branch_a = repo.branch("branch-a", &init, false).unwrap();

            let a1 = commit("refs/heads/branch-a", "test A1", &tree, &[&init], &repo);
            let _a2 = commit("refs/heads/branch-a", "test A2", &tree, &[&a1], &repo);

            let _branch_b = repo.branch("branch-b", &a1, false).unwrap();

            let b1 = commit("refs/heads/branch-b", "test B1", &tree, &[&a1], &repo);
            let _b2 = commit("refs/heads/branch-b", "test B2", &tree, &[&b1], &repo);

            let _branch_c = repo.branch("branch-c", &b1, false).unwrap();

            let c1 = commit("refs/heads/branch-c", "test C1", &tree, &[&b1], &repo);
            let _c2 = commit("refs/heads/branch-c", "test C2", &tree, &[&c1], &repo);
        }

        repo
    }

    fn test_repo() -> Repository {
        let dir = Builder::new().prefix("git-test").tempdir().unwrap();
        Repository::init(dir.into_path()).unwrap()
    }

    fn commit<'repo>(
        update_ref: &str,
        message: &str,
        tree: &Tree,
        parents: &[&Commit<'_>],
        repo: &'repo Repository,
    ) -> Commit<'repo> {
        let author = Signature::now("test", "test@dylowen.com").unwrap();

        let id = repo
            .commit(Some(update_ref), &author, &author, message, tree, parents)
            .unwrap();

        repo.find_commit(id).unwrap()
    }
}
