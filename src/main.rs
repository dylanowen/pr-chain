extern crate core;

mod git;
mod pr_chain;

use crate::git::fetch_remotes;
use crate::pr_chain::PrChain;
use anyhow::Context;
use clap::{Parser, Subcommand};
use git2::Repository;

const _GITHUB_URL: &str = "https://api.github.com";

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
#[clap(propagate_version = true)]
struct GHStackArgs {
    #[clap(subcommand)]
    command: Commands,
    /// Set the environment log level
    #[clap(long, env = env_logger::DEFAULT_FILTER_ENV, default_value_t = String::from("info"))]
    log_level: String,
    /// Set the environment log style
    #[clap(long, env = env_logger::DEFAULT_WRITE_STYLE_ENV)]
    log_style: Option<String>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Annotate the descriptions of all PRs in a stack with metadata about all PRs in the stack
    // Annotate {
    //     #[clap(flatten)]
    //     standard_args: StandardArgs,
    // },
    Log {
        // #[clap(flatten)]
        // standard_args: StandardArgs,
        /// The last branch in the chain that we're going to logging
        #[clap(index(1))]
        branch: String,
        #[clap(long, default_value = "main")]
        trunk: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = GHStackArgs::parse();
    setup_logging(&args)?;

    let repo = Repository::discover(".").context("Couldn't find a repository")?;
    fetch_remotes(&repo)?;

    match args.command {
        Commands::Log {
            branch: branch_name,
            trunk: trunk_name,
        } => {
            PrChain::init(&repo, &branch_name, &trunk_name)?
                .log_plan(&repo)
                .await?;

            // let trunk = repo
            //     .find_branch(&trunk_name, BranchType::Local)
            //     .with_context(|| format!("Couldn't find branch '{trunk_name}'"))?;
            // let branch = repo
            //     .find_branch(&branch_name, BranchType::Local)
            //     .with_context(|| format!("Couldn't find branch '{branch_name}'"))?;
            // let trunk_id = trunk
            //     .get()
            //     .target()
            //     .ok_or_else(|| anyhow!("Couldn't find a reference for {trunk_name}"))?;
            // let branch_id = branch
            //     .get()
            //     .target()
            //     .ok_or_else(|| anyhow!("Couldn't find a reference for {branch_name}"))?;
            //
            // let other_branches = repo
            //     .branches(None)?
            //     .into_iter()
            //     .filter_map(|branch| branch.ok())
            //     .filter(|(other_branch, _branch_type)| {
            //         other_branch.get() != trunk.get() && other_branch.get() != branch.get()
            //     })
            //     .collect::<Vec<_>>();
            //
            // for (other_branch, _) in other_branches {
            //     println!("{:?}", other_branch.name());
            // }
            //
            // let merge_base = repo.merge_base(trunk_id, branch_id).with_context(|| {
            //     format!("Couldn't find a merge base of '{branch_name}' and '{trunk_name}'")
            // })?;
            //
            // git::log(&format!("{merge_base}")).await;
            //
            // let mut walk = repo.revwalk()?;
            // walk.push(branch_id);
            // walk.hide(trunk_id);
            //
            // let ids_to_base = walk.into_iter().collect::<Result<Vec<_>, _>>()?;
            //
            // for id in ids_to_base {
            //     println!("{id:?}");
            // }
            //
            // println!("{:?} {:?}", trunk.get().target(), merge_base);
        }
    }

    Ok(())
}

fn setup_logging(args: &GHStackArgs) -> anyhow::Result<()> {
    let mut builder = env_logger::Builder::new();
    builder.parse_filters(&args.log_level);

    if let Some(s) = &args.log_style {
        builder.parse_write_style(s);
    }

    builder.try_init().context("Failed to setup logger")
}
