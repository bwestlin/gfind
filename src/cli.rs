use std::env;
use std::ffi::OsString;
use std::io;
use std::path::PathBuf;

use clap::Command;
use clap::CommandFactory;
use clap::FromArgMatches;
use clap::Parser;
use clap::Subcommand;
use clap_complete::Shell;
use clap_complete::generate;

use crate::config::ConfigCommands;
use crate::config::ConfigFile;
use crate::config::DEFAULT_EXCLUDE_DIR;
use crate::config::load_optional_config;
use crate::logger::Logger;
use crate::query::QueryMode;

#[derive(Debug, Parser)]
#[command(version, about = "Find things across Git repos")]
pub(crate) struct Cli {
    /// Print diagnostic progress to stderr.
    #[arg(short, long, global = true)]
    pub(crate) verbose: bool,

    /// Print absolute repo paths instead of paths relative to cwd.
    #[arg(
        long,
        alias = "absolute-paths",
        global = true,
        conflicts_with = "relative"
    )]
    pub(crate) absolute: bool,

    /// Print repo paths relative to cwd, overriding config.
    #[arg(long, alias = "relative-paths", global = true)]
    pub(crate) relative: bool,

    /// Path to a TOML config file.
    #[arg(long, global = true, default_value = "~/.gfind/config.toml")]
    pub(crate) config: PathBuf,

    /// Search under this path instead of configured paths. Repeatable.
    #[arg(short = 'p', long = "path", global = true)]
    pub(crate) paths: Vec<PathBuf>,

    /// Include the current working directory in the search roots.
    #[arg(long, global = true, conflicts_with = "no_cwd")]
    pub(crate) cwd: bool,

    /// Do not include the current working directory in the search roots.
    #[arg(long, global = true)]
    pub(crate) no_cwd: bool,

    /// Exclude matching directories from repo traversal. Repeatable.
    #[arg(
        long = "exclude-dir",
        global = true,
        value_name = "REGEX",
        conflicts_with = "no_exclude_dirs"
    )]
    pub(crate) exclude_dirs: Vec<String>,

    /// Disable configured and default directory exclusions.
    #[arg(long, global = true)]
    pub(crate) no_exclude_dirs: bool,

    #[command(subcommand)]
    pub(crate) command: Commands,
}

impl Cli {
    pub(crate) fn parse() -> Self {
        let mut args: Vec<OsString> = env::args_os().collect();

        if !requests_help(&args) {
            return <Self as Parser>::parse_from(args);
        }

        normalize_help_command(&mut args);
        let config_path = config_path_from_args(&args);
        let config = load_optional_config(&config_path, Logger::new(false))
            .ok()
            .flatten();
        let help_scope = HelpScope::from_args(&args);
        let command = command_with_help_defaults(config.as_ref(), help_scope);

        match command.try_get_matches_from(args) {
            Ok(matches) => Self::from_arg_matches(&matches).unwrap_or_else(|err| err.exit()),
            Err(err) => err.exit(),
        }
    }
}

#[derive(Debug, Subcommand)]
pub(crate) enum Commands {
    /// Generate shell completion scripts.
    #[command(visible_alias = "comp")]
    Completions {
        /// Shell to generate completions for.
        shell: Shell,
    },

    /// Create or inspect gfind configuration.
    #[command(visible_alias = "cfg")]
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },

    /// List repos, optionally filtered by path or remote URL.
    #[command(visible_alias = "r")]
    Repos {
        /// Case-insensitive text to match against repo paths and remote URLs.
        query: Option<String>,

        #[command(flatten)]
        query_options: QueryOptions,
    },

    /// Find repos that have a local branch by default.
    #[command(visible_alias = "b")]
    Branch {
        /// Branch query to search for.
        query: String,

        #[command(flatten)]
        query_options: QueryOptions,

        /// Print full matching branch commit hashes. Useful to override config.
        #[arg(
            long,
            alias = "full-commit-hash",
            conflicts_with_all = ["no_commit_hash", "short_commit_hash"]
        )]
        commit_hash: bool,

        /// Print short matching branch commit hashes.
        #[arg(long, conflicts_with_all = ["commit_hash", "no_commit_hash"])]
        short_commit_hash: bool,

        /// Do not print matching branch commit hashes.
        #[arg(long, conflicts_with_all = ["commit_hash", "short_commit_hash"])]
        no_commit_hash: bool,

        /// Print local branch upstream freshness. Useful to override config.
        #[arg(long, alias = "remote-status", conflicts_with = "no_upstream_status")]
        upstream_status: bool,

        /// Do not print local branch upstream freshness.
        #[arg(long, alias = "no-remote-status")]
        no_upstream_status: bool,

        /// Prefer checking the live origin remote branch before local tracking refs.
        #[arg(
            long,
            alias = "remote-origin-status",
            conflicts_with = "no_origin_status"
        )]
        origin_status: bool,

        /// Do not check the live origin remote branch for freshness.
        #[arg(long, alias = "no-remote-origin-status")]
        no_origin_status: bool,

        /// Only match repos where this branch is currently checked out.
        #[arg(long)]
        current: bool,

        /// Match local branch refs. Useful to override config.
        #[arg(long)]
        local: bool,

        /// Match remote branch refs.
        #[arg(long)]
        remote: bool,

        /// Match local and remote branch refs.
        #[arg(long)]
        all: bool,
    },

    /// Find repos that contain a commit object.
    #[command(visible_alias = "c")]
    Commit {
        /// Commit SHA or revision to search for.
        rev: String,

        /// Only match repos where the commit is reachable from HEAD.
        #[arg(long)]
        reachable: bool,
    },

    /// List repos with a dirty worktree and/or unpushed commits.
    #[command(visible_alias = "s")]
    Status {
        /// Only report dirty worktrees.
        #[arg(long, conflicts_with = "unpushed_only")]
        dirty_only: bool,

        /// Only report branches with commits ahead of their upstream.
        #[arg(long)]
        unpushed_only: bool,
    },

    /// List, search, and print stashes across repos.
    #[command(visible_alias = "sh")]
    Stash {
        /// Case-insensitive text to match against stash refs and subjects.
        query: Option<String>,

        #[command(flatten)]
        query_options: QueryOptions,

        /// Print matching stash contents with `git stash show`.
        #[arg(long)]
        print: bool,

        /// Print matching stash patches.
        #[arg(long)]
        patch: bool,

        /// Select a specific stash ref, for example `stash@{0}` or `0`.
        #[arg(long)]
        stash: Option<String>,
    },
}

pub(crate) fn print_completions(shell: Shell) {
    let mut command = Cli::command();
    generate(shell, &mut command, "gfind", &mut io::stdout());
}

fn requests_help(args: &[OsString]) -> bool {
    args.iter()
        .skip(1)
        .any(|arg| arg == "-h" || arg == "--help")
        || top_level_command_index(args).is_some_and(|index| args[index] == "help")
}

fn normalize_help_command(args: &mut Vec<OsString>) {
    let Some(index) = top_level_command_index(args) else {
        return;
    };

    if args[index] == "help" {
        args.remove(index);
        args.push(OsString::from("--help"));
    }
}

fn top_level_command_index(args: &[OsString]) -> Option<usize> {
    let mut index = 1;

    while index < args.len() {
        let arg = args[index].to_string_lossy();

        if matches!(arg.as_ref(), "--config" | "--path" | "-p" | "--exclude-dir") {
            index += 2;
        } else if arg.starts_with("--config=")
            || arg.starts_with("--path=")
            || arg.starts_with("--exclude-dir=")
            || arg.starts_with('-')
        {
            index += 1;
        } else {
            return Some(index);
        }
    }

    None
}

fn config_path_from_args(args: &[OsString]) -> PathBuf {
    let mut args = args.iter().skip(1);

    while let Some(arg) = args.next() {
        if arg == "--config" {
            return args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(default_config_path);
        }

        if let Some(arg) = arg.to_str()
            && let Some(path) = arg.strip_prefix("--config=")
        {
            return PathBuf::from(path);
        }
    }

    default_config_path()
}

fn default_config_path() -> PathBuf {
    PathBuf::from("~/.gfind/config.toml")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HelpScope {
    TopLevel,
    Repos,
    Branch,
    Commit,
    Status,
    Stash,
    Other,
}

impl HelpScope {
    fn from_args(args: &[OsString]) -> Self {
        let Some(index) = top_level_command_index(args) else {
            return Self::TopLevel;
        };

        match args[index].to_string_lossy().as_ref() {
            "repos" | "r" => Self::Repos,
            "branch" | "b" => Self::Branch,
            "commit" | "c" => Self::Commit,
            "status" | "s" => Self::Status,
            "stash" | "sh" => Self::Stash,
            _ => Self::Other,
        }
    }

    fn uses_traversal(self) -> bool {
        matches!(
            self,
            Self::TopLevel | Self::Repos | Self::Branch | Self::Commit | Self::Status | Self::Stash
        )
    }
}

fn command_with_help_defaults(config: Option<&ConfigFile>, scope: HelpScope) -> Command {
    let defaults = ConfigFile::default();
    let effective = config.unwrap_or(&defaults);
    let command = apply_traversal_defaults(Cli::command(), effective)
        .mut_subcommand("repos", |command| {
            command.mut_arg("query_mode", |arg| {
                arg.default_value(effective.effective_repo_query().as_str())
            })
        })
        .mut_subcommand("branch", |command| {
            let command = command.mut_arg("query_mode", |arg| {
                arg.default_value(effective.effective_branch_query().as_str())
            });
            let command = set_flag_default(
                command,
                "upstream_status",
                effective.effective_branch_upstream_status(),
            );
            let command = set_flag_default(
                command,
                "origin_status",
                effective.effective_branch_origin_status(),
            );
            let command = set_selected_flag_default(
                command,
                &["current", "local", "remote", "all"],
                effective.effective_branch_search().as_str(),
            );

            set_selected_flag_default(
                command,
                &["no_commit_hash", "short_commit_hash", "commit_hash"],
                match effective.effective_branch_hash() {
                    crate::config::BranchHash::None => "no_commit_hash",
                    crate::config::BranchHash::Short => "short_commit_hash",
                    crate::config::BranchHash::Full => "commit_hash",
                },
            )
        })
        .mut_subcommand("stash", |command| {
            command.mut_arg("query_mode", |arg| {
                arg.default_value(effective.effective_stash_query().as_str())
            })
        });

    let Some(footer) = config.and_then(|config| effective_config_footer(config, scope)) else {
        return command;
    };

    match scope {
        HelpScope::TopLevel => command.after_long_help(footer),
        HelpScope::Repos => {
            command.mut_subcommand("repos", |command| command.after_long_help(footer))
        }
        HelpScope::Branch => {
            command.mut_subcommand("branch", |command| command.after_long_help(footer))
        }
        HelpScope::Commit => {
            command.mut_subcommand("commit", |command| command.after_long_help(footer))
        }
        HelpScope::Status => {
            command.mut_subcommand("status", |command| command.after_long_help(footer))
        }
        HelpScope::Stash => {
            command.mut_subcommand("stash", |command| command.after_long_help(footer))
        }
        HelpScope::Other => command,
    }
}

fn apply_traversal_defaults(mut command: Command, config: &ConfigFile) -> Command {
    command = set_flag_default(command, "cwd", config.effective_include_cwd());
    command = set_flag_default(command, "absolute", config.effective_absolute_paths());

    if let Some(paths) = config.paths.as_ref().filter(|paths| !paths.is_empty()) {
        let paths = paths
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>();
        command = command.mut_arg("paths", |arg| arg.default_values(paths));
    }

    let exclusions = config.effective_exclude_dirs();
    if exclusions.is_empty() {
        command.mut_arg("no_exclude_dirs", |arg| arg.default_value("true"))
    } else {
        command.mut_arg("exclude_dirs", |arg| arg.default_values(exclusions))
    }
}

fn set_selected_flag_default(mut command: Command, ids: &[&str], selected: &str) -> Command {
    for id in ids {
        if *id == selected {
            command = set_flag_default(command, id, true);
        }
    }

    command
}

fn set_flag_default(command: Command, id: &str, value: bool) -> Command {
    command.mut_arg(id, |arg| {
        let help = arg.get_help().map(ToString::to_string).unwrap_or_default();

        // Clap only renders native defaults for value-taking arguments.
        arg.default_value(value.to_string())
            .help(format!("{help} [default: {value}]"))
    })
}

fn effective_config_footer(config: &ConfigFile, scope: HelpScope) -> Option<String> {
    let mut overrides = Vec::new();

    if scope.uses_traversal() {
        if let Some(paths) = config.paths.as_ref().filter(|paths| !paths.is_empty()) {
            overrides.extend(
                paths
                    .iter()
                    .map(|path| format!("--path {}", path.display())),
            );
        }
        if !config.effective_include_cwd() {
            overrides.push("--no-cwd".to_string());
        }
        if config.effective_absolute_paths() {
            overrides.push("--absolute".to_string());
        }
        if config.effective_exclude_dirs() != [DEFAULT_EXCLUDE_DIR] {
            let exclusions = config.effective_exclude_dirs();
            if exclusions.is_empty() {
                overrides.push("--no-exclude-dirs".to_string());
            } else {
                overrides.extend(
                    exclusions
                        .into_iter()
                        .map(|exclusion| format!("--exclude-dir {exclusion}")),
                );
            }
        }
    }

    match scope {
        HelpScope::Repos if config.effective_repo_query() != QueryMode::Contains => {
            overrides.push(format!(
                "--query-mode {}",
                config.effective_repo_query().as_str()
            ));
        }
        HelpScope::Branch => {
            if config.effective_branch_query() != QueryMode::Contains {
                overrides.push(format!(
                    "--query-mode {}",
                    config.effective_branch_query().as_str()
                ));
            }
            if config.effective_branch_search() != crate::config::BranchSearch::Local {
                overrides.push(format!("--{}", config.effective_branch_search().as_str()));
            }
            if config.effective_branch_hash() != crate::config::BranchHash::Full {
                overrides.push(
                    match config.effective_branch_hash() {
                        crate::config::BranchHash::None => "--no-commit-hash",
                        crate::config::BranchHash::Short => "--short-commit-hash",
                        crate::config::BranchHash::Full => "--commit-hash",
                    }
                    .to_string(),
                );
            }
            if !config.effective_branch_upstream_status() {
                overrides.push("--no-upstream-status".to_string());
            }
            if config.effective_branch_origin_status() {
                overrides.push("--origin-status".to_string());
            }
        }
        HelpScope::Stash if config.effective_stash_query() != QueryMode::Contains => {
            overrides.push(format!(
                "--query-mode {}",
                config.effective_stash_query().as_str()
            ));
        }
        _ => {}
    }

    if overrides.is_empty() {
        None
    } else {
        Some(format!(
            "Config overrides (equivalent CLI flags):\n  {}",
            overrides.join("\n  ")
        ))
    }
}

#[derive(Clone, Debug, Default, clap::Args)]
pub(crate) struct QueryOptions {
    /// Query mode: contains, matches, exact, fuzzy, or regex.
    #[arg(long, value_enum, conflicts_with_all = ["contains", "exact", "fuzzy", "regex"])]
    pub(crate) query_mode: Option<QueryMode>,

    /// Case-insensitive substring matching.
    #[arg(long, alias = "matches")]
    pub(crate) contains: bool,

    /// Exact case-insensitive matching.
    #[arg(long, conflicts_with_all = ["contains", "fuzzy", "regex"])]
    pub(crate) exact: bool,

    /// Ordered character fuzzy matching.
    #[arg(long, conflicts_with_all = ["contains", "exact", "regex"])]
    pub(crate) fuzzy: bool,

    /// Regular expression matching.
    #[arg(long, conflicts_with_all = ["contains", "exact", "fuzzy"])]
    pub(crate) regex: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::BranchHash;
    use crate::config::BranchSearch;
    use crate::config::documented_config;

    #[test]
    fn parses_completion_shell() {
        let cli = Cli::try_parse_from(["gfind", "completions", "bash"]).unwrap();

        assert!(matches!(
            cli.command,
            Commands::Completions { shell: Shell::Bash }
        ));
    }

    #[test]
    fn parses_top_level_command_aliases() {
        assert!(matches!(
            Cli::try_parse_from(["gfind", "r"]).unwrap().command,
            Commands::Repos { .. }
        ));
        assert!(matches!(
            Cli::try_parse_from(["gfind", "b", "main"]).unwrap().command,
            Commands::Branch { .. }
        ));
        assert!(matches!(
            Cli::try_parse_from(["gfind", "c", "HEAD"]).unwrap().command,
            Commands::Commit { .. }
        ));
        assert!(matches!(
            Cli::try_parse_from(["gfind", "s"]).unwrap().command,
            Commands::Status { .. }
        ));
        assert!(matches!(
            Cli::try_parse_from(["gfind", "sh"]).unwrap().command,
            Commands::Stash { .. }
        ));
        assert!(matches!(
            Cli::try_parse_from(["gfind", "cfg", "init"])
                .unwrap()
                .command,
            Commands::Config { .. }
        ));
        assert!(matches!(
            Cli::try_parse_from(["gfind", "comp", "bash"])
                .unwrap()
                .command,
            Commands::Completions { shell: Shell::Bash }
        ));
    }

    #[test]
    fn generates_completion_script() {
        let mut command = Cli::command();
        let mut output = Vec::new();

        generate(Shell::Bash, &mut command, "gfind", &mut output);

        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("_gfind"));
        assert!(output.contains("completions"));
    }

    #[test]
    fn parses_directory_exclusions() {
        let cli = Cli::try_parse_from([
            "gfind",
            "--exclude-dir",
            "^vendor$",
            "--exclude-dir",
            "^build/",
            "repos",
        ])
        .unwrap();

        assert_eq!(cli.exclude_dirs, ["^vendor$", "^build/"]);
        assert!(!cli.no_exclude_dirs);
    }

    #[test]
    fn finds_config_path_for_help() {
        assert_eq!(
            config_path_from_args(&[
                OsString::from("gfind"),
                OsString::from("branch"),
                OsString::from("--config"),
                OsString::from("./custom.toml"),
                OsString::from("--help"),
            ]),
            PathBuf::from("./custom.toml")
        );
        assert_eq!(
            config_path_from_args(&[
                OsString::from("gfind"),
                OsString::from("--config=./other.toml"),
                OsString::from("--help"),
            ]),
            PathBuf::from("./other.toml")
        );
    }

    #[test]
    fn normalizes_help_subcommand_after_global_options() {
        let mut args = vec![
            OsString::from("gfind"),
            OsString::from("--config"),
            OsString::from("./custom.toml"),
            OsString::from("help"),
            OsString::from("branch"),
        ];

        assert!(requests_help(&args));
        normalize_help_command(&mut args);

        assert_eq!(
            args,
            ["gfind", "--config", "./custom.toml", "branch", "--help"].map(OsString::from)
        );
    }

    #[test]
    fn help_shows_all_relevant_builtin_defaults_without_config_overrides() {
        let branch_help = command_with_help_defaults(None, HelpScope::Branch)
            .try_get_matches_from(["gfind", "branch", "--help"])
            .unwrap_err()
            .to_string();

        assert!(
            branch_help.contains(
                "Include the current working directory in the search roots [default: true]"
            ),
            "{branch_help}"
        );
        assert!(branch_help.contains(
            "Print absolute repo paths instead of paths relative to cwd [default: false]"
        ));
        assert!(branch_help.contains("[default: ^target$]"));
        assert!(branch_help.contains("[default: contains]"));
        assert!(
            branch_help
                .contains("Match local branch refs. Useful to override config [default: true]")
        );
        assert!(branch_help.contains(
            "Print full matching branch commit hashes. Useful to override config [default: true]"
        ));
        assert!(branch_help.contains(
            "Print local branch upstream freshness. Useful to override config [default: true]"
        ));
        assert!(branch_help.contains(
            "Prefer checking the live origin remote branch before local tracking refs [default: false]"
        ));
        assert!(!branch_help.contains("Defaults:"));
        assert!(!branch_help.contains("Config overrides (equivalent CLI flags):"));
    }

    #[test]
    fn help_omits_footer_for_default_equivalent_config() {
        let config: ConfigFile = toml::from_str(documented_config()).unwrap();
        let configured_help = command_with_help_defaults(Some(&config), HelpScope::Branch)
            .try_get_matches_from(["gfind", "branch", "--help"])
            .unwrap_err()
            .to_string();
        let default_help = command_with_help_defaults(None, HelpScope::Branch)
            .try_get_matches_from(["gfind", "branch", "--help"])
            .unwrap_err()
            .to_string();

        assert_eq!(configured_help, default_help);
    }

    #[test]
    fn help_renders_config_overrides_in_scoped_footer() {
        let config = ConfigFile {
            paths: Some(vec![PathBuf::from("~/src")]),
            include_cwd: Some(false),
            absolute_paths: Some(true),
            exclude_dirs: Some(vec!["^vendor$".to_string()]),
            branch_search: Some(BranchSearch::All),
            branch_hash: Some(BranchHash::Short),
            branch_upstream_status: Some(false),
            branch_origin_status: Some(true),
            branch_query: Some(QueryMode::Regex),
            ..ConfigFile::default()
        };
        let top_level_help = command_with_help_defaults(Some(&config), HelpScope::TopLevel)
            .try_get_matches_from(["gfind", "--help"])
            .unwrap_err()
            .to_string();
        let branch_help = command_with_help_defaults(Some(&config), HelpScope::Branch)
            .try_get_matches_from(["gfind", "branch", "--help"])
            .unwrap_err()
            .to_string();

        assert!(top_level_help.contains("[default: ~/src]"));
        assert!(top_level_help.contains(
            "Include the current working directory in the search roots [default: false]"
        ));
        assert!(top_level_help.contains(
            "Print absolute repo paths instead of paths relative to cwd [default: true]"
        ));
        assert!(top_level_help.contains("[default: ^vendor$]"));
        assert!(top_level_help.contains("Config overrides (equivalent CLI flags):"));
        assert!(top_level_help.contains("--path ~/src"));
        assert!(top_level_help.contains("--no-cwd"));
        assert!(top_level_help.contains("--absolute"));
        assert!(top_level_help.contains("--exclude-dir ^vendor$"));
        assert!(!top_level_help.contains("--all"));

        assert!(branch_help.contains("[default: regex]"));
        assert!(branch_help.contains("Match local and remote branch refs [default: true]"));
        assert!(branch_help.contains("Print short matching branch commit hashes [default: true]"));
        assert!(branch_help.contains("Config overrides (equivalent CLI flags):"));
        assert!(branch_help.contains("--query-mode regex"));
        assert!(branch_help.contains("--all"));
        assert!(branch_help.contains("--short-commit-hash"));
        assert!(branch_help.contains("--no-upstream-status"));
        assert!(branch_help.contains("--origin-status"));
    }

    #[test]
    fn uses_config_toml_in_gfind_directory_by_default() {
        let cli = Cli::try_parse_from(["gfind", "repos"]).unwrap();

        assert_eq!(cli.config, PathBuf::from("~/.gfind/config.toml"));
    }
}
