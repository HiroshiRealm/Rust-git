use anyhow::Result;
use clap::{Parser, Subcommand};
mod commands;
mod repository;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new Git repository
    Init,
    
    /// Add file contents to the index
    Add {
        /// Files to add
        #[arg(required = true)]
        paths: Vec<String>,
    },
    
    /// Remove files from the working tree and index
    Rm {
        /// Files to remove
        #[arg(required = true)]
        paths: Vec<String>,
    },
    
    /// Record changes to the repository
    Commit {
        /// Commit message
        #[arg(short = 'm', long, required = true)]
        message: String,
    },
    
    /// List, create, or delete branches
    Branch {
        /// Branch name
        name: Option<String>,
        
        /// Delete the branch
        #[arg(short, long)]
        delete: bool,
    },
    
    /// Switch branches or restore working tree files
    Checkout {
        /// Branch to checkout or create
        branch: String,

        /// Create a new branch and switch to it
        #[arg(short = 'b', long = "branch", required = false)]
        create_branch: bool,
    },
    
    /// Join two or more development histories together
    Merge {
        /// Branch to merge
        branch: String,
    },
    
    /// Download objects and refs from another repository
    Fetch {
        /// Repository to fetch from
        remote: String,
    },
    
    /// Fetch from and integrate with another repository
    Pull {
        /// Repository to pull from
        remote: String,
    },
    
    /// Update remote refs along with associated objects
    Push {
        /// Repository to push to
        remote: String,
    },

    /// Pretty-print Git objects
    CatFile {
        /// The object to display
        #[arg(name = "object")]
        object_hash: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Init => commands::init::execute()?,
        Commands::Add { paths } => commands::add::execute(paths)?,
        Commands::Rm { paths } => commands::rm::execute(paths)?,
        Commands::Commit { message } => commands::commit::execute(message)?,
        Commands::Branch { name, delete } => commands::branch::execute(name.as_deref(), *delete)?,
        Commands::Checkout { branch, create_branch } => commands::checkout::execute(branch, *create_branch)?,
        Commands::Merge { branch } => commands::merge::execute(branch)?,
        Commands::Fetch { remote } => commands::fetch::execute(remote)?,
        Commands::Pull { remote } => commands::pull::execute(remote)?,
        Commands::Push { remote } => commands::push::execute(remote)?,
        Commands::CatFile { object_hash } => commands::cat_file::execute(object_hash)?,
    }
    
    Ok(())
}
