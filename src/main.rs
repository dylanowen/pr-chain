use anyhow::{anyhow, Context};
use clap::{App, AppSettings, Arg, ArgMatches, Args, Parser, Subcommand};
use git2::{BranchType, Repository};

const GITHUB_URL: &str = "https://api.github.com";

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
        base: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = GHStackArgs::parse();
    setup_logging(&args)?;

    let repo = Repository::discover(".").context("Couldn't find a repository")?;

    match args.command {
        Commands::Log {
            branch: branch_name,
            base: base_name,
        } => {
            let base = repo
                .find_branch(&base_name, BranchType::Local)
                .with_context(|| format!("Couldn't find branch '{base_name}'"))?;
            let branch = repo
                .find_branch(&branch_name, BranchType::Local)
                .with_context(|| format!("Couldn't find branch '{branch_name}'"))?;
            let base_id = base
                .get()
                .target()
                .ok_or_else(|| anyhow!("Couldn't find a reference for {base_name}"))?;
            let branch_id = branch
                .get()
                .target()
                .ok_or_else(|| anyhow!("Couldn't find a reference for {branch_name}"))?;

            let merge_base = repo.merge_base(base_id, branch_id);

            println!("{:?} {:?}", base.get().target(), merge_base);
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
