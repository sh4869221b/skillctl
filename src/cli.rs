use std::process::ExitCode;

use clap::{ArgGroup, Parser, Subcommand};

use crate::config::Config;
use crate::diff::run_diff;
use crate::error::{AppError, AppResult};
use crate::status::{list_skills, render_status_table, status_for_target};
use crate::sync::{Selection, execute_plan, plan_import, plan_push, summarize_plan};

#[derive(Debug, Parser)]
#[command(name = "skillctl", version, about = "skill sync CLI")]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Targets,
    #[command(group(
        ArgGroup::new("scope")
            .required(true)
            .args(["global", "target"])
    ))]
    List {
        #[arg(long)]
        global: bool,
        #[arg(long)]
        target: Option<String>,
    },
    #[command(group(
        ArgGroup::new("scope")
            .required(true)
            .args(["target", "all"])
    ))]
    Status {
        #[arg(long)]
        target: Option<String>,
        #[arg(long)]
        all: bool,
    },
    #[command(group(
        ArgGroup::new("selection")
            .required(true)
            .args(["skill", "all"])
    ))]
    Push {
        skill: Option<String>,
        #[arg(long)]
        all: bool,
        #[arg(long)]
        target: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        prune: bool,
    },
    #[command(group(
        ArgGroup::new("selection")
            .required(true)
            .args(["skill", "all"])
    ))]
    Import {
        skill: Option<String>,
        #[arg(long)]
        all: bool,
        #[arg(long)]
        from: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        overwrite: bool,
    },
    Diff {
        skill: String,
        #[arg(long)]
        target: String,
    },
}

pub fn run() -> ExitCode {
    let cli = Cli::parse();
    match execute(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {}", err);
            if let Some(hint) = err.hint() {
                eprintln!("help: {}", hint);
            }
            err.exit_code()
        }
    }
}

fn execute(cli: Cli) -> AppResult<()> {
    let config = Config::load_default()?;
    match cli.command {
        Commands::Targets => {
            for target in &config.targets {
                println!("{}", target.name);
            }
        }
        Commands::List { global, target } => {
            let root = if global {
                &config.global_root
            } else {
                let name = target.ok_or_else(|| {
                    AppError::config(
                        "target が指定されていません".to_string(),
                        Some("list --target <name> を指定してください".to_string()),
                    )
                })?;
                &config.target_by_name(&name)?.root
            };
            let skills = list_skills(root)?;
            for skill in skills {
                println!("{}", skill);
            }
        }
        Commands::Status { target, all } => {
            if all {
                for t in &config.targets {
                    println!("Target: {}", t.name);
                    let rows = status_for_target(&config, t)?;
                    let table = render_status_table(&rows)?;
                    print!("{}", table);
                }
            } else {
                let name = target.ok_or_else(|| {
                    AppError::config(
                        "target が指定されていません".to_string(),
                        Some("status --target <name> を指定してください".to_string()),
                    )
                })?;
                let target = config.target_by_name(&name)?;
                let rows = status_for_target(&config, target)?;
                let table = render_status_table(&rows)?;
                print!("{}", table);
            }
        }
        Commands::Push {
            skill,
            all,
            target,
            dry_run,
            prune,
        } => {
            let target = config.target_by_name(&target)?;
            let selection = if all {
                Selection::All
            } else {
                Selection::One(skill.as_deref().ok_or_else(|| {
                    AppError::config(
                        "skill が指定されていません".to_string(),
                        Some("push <skill> を指定してください".to_string()),
                    )
                })?)
            };
            let plan = plan_push(&config, target, selection, prune)?;
            for line in summarize_plan(&plan) {
                println!("{}", line);
            }
            execute_plan(&plan, dry_run)?;
        }
        Commands::Import {
            skill,
            all,
            from,
            dry_run,
            overwrite,
        } => {
            let target = config.target_by_name(&from)?;
            let selection = if all {
                Selection::All
            } else {
                Selection::One(skill.as_deref().ok_or_else(|| {
                    AppError::config(
                        "skill が指定されていません".to_string(),
                        Some("import <skill> を指定してください".to_string()),
                    )
                })?)
            };
            let plan = plan_import(&config, target, selection, overwrite)?;
            for line in summarize_plan(&plan) {
                println!("{}", line);
            }
            execute_plan(&plan, dry_run)?;
        }
        Commands::Diff { skill, target } => {
            let target = config.target_by_name(&target)?;
            run_diff(&config, target, &skill)?;
        }
    }
    Ok(())
}
