use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use clap::Subcommand;
use clap::ValueEnum;
use serde::Deserialize;

use crate::cli::Cli;
use crate::error::Result;
use crate::logger::Logger;
use crate::query::QueryMode;

pub(crate) const DEFAULT_EXCLUDE_DIR: &str = "^target$";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum BranchSearch {
    Current,
    Local,
    Remote,
    All,
}

impl BranchSearch {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Local => "local",
            Self::Remote => "remote",
            Self::All => "all",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum BranchHash {
    None,
    Short,
    Full,
}

#[derive(Debug, Subcommand)]
pub(crate) enum ConfigCommands {
    /// Create a documented config file.
    Init {
        /// Overwrite an existing config file.
        #[arg(long)]
        force: bool,
    },
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct ConfigFile {
    /// Paths to search when `--path` is not provided.
    pub(crate) paths: Option<Vec<PathBuf>>,

    /// Whether cwd should be included in search roots. Defaults to true.
    pub(crate) include_cwd: Option<bool>,

    /// Whether repo paths should be printed as absolute paths. Defaults to false.
    pub(crate) absolute_paths: Option<bool>,

    /// Directory regexes excluded from repo traversal. Defaults to ^target$.
    pub(crate) exclude_dirs: Option<Vec<String>>,

    /// Default branch search mode: current, local, remote, or all. Defaults to local.
    pub(crate) branch_search: Option<BranchSearch>,

    /// Branch hash output mode: none, short, or full. Defaults to full.
    pub(crate) branch_hash: Option<BranchHash>,

    /// Whether local branch results should include upstream freshness. Defaults to true.
    pub(crate) branch_upstream_status: Option<bool>,

    /// Whether branch freshness should prefer the live origin remote. Defaults to false.
    pub(crate) branch_origin_status: Option<bool>,

    /// Deprecated compatibility setting. Use branch_hash instead.
    pub(crate) branch_hashes: Option<bool>,

    /// Default repo query mode: contains, matches, exact, fuzzy, or regex.
    pub(crate) repo_query: Option<QueryMode>,

    /// Default branch query mode: contains, matches, exact, fuzzy, or regex.
    pub(crate) branch_query: Option<QueryMode>,

    /// Default stash query mode: contains, matches, exact, fuzzy, or regex.
    pub(crate) stash_query: Option<QueryMode>,
}

impl ConfigFile {
    pub(crate) fn effective_include_cwd(&self) -> bool {
        self.include_cwd.unwrap_or(true)
    }

    pub(crate) fn effective_absolute_paths(&self) -> bool {
        self.absolute_paths.unwrap_or(false)
    }

    pub(crate) fn effective_exclude_dirs(&self) -> Vec<String> {
        self.exclude_dirs
            .clone()
            .unwrap_or_else(|| vec![DEFAULT_EXCLUDE_DIR.to_string()])
    }

    pub(crate) fn effective_branch_search(&self) -> BranchSearch {
        self.branch_search.unwrap_or(BranchSearch::Local)
    }

    pub(crate) fn effective_branch_hash(&self) -> BranchHash {
        if let Some(branch_hash) = self.branch_hash {
            branch_hash
        } else if self.branch_hashes == Some(false) {
            BranchHash::None
        } else {
            BranchHash::Full
        }
    }

    pub(crate) fn effective_branch_upstream_status(&self) -> bool {
        self.branch_upstream_status.unwrap_or(true)
    }

    pub(crate) fn effective_branch_origin_status(&self) -> bool {
        self.branch_origin_status.unwrap_or(false)
    }

    pub(crate) fn effective_repo_query(&self) -> QueryMode {
        self.repo_query.unwrap_or(QueryMode::Contains)
    }

    pub(crate) fn effective_branch_query(&self) -> QueryMode {
        self.branch_query.unwrap_or(QueryMode::Contains)
    }

    pub(crate) fn effective_stash_query(&self) -> QueryMode {
        self.stash_query.unwrap_or(QueryMode::Contains)
    }
}

pub(crate) fn run_config_command(command: &ConfigCommands, config_path: &Path) -> Result<()> {
    match command {
        ConfigCommands::Init { force } => init_config(config_path, *force),
    }
}

pub(crate) fn init_config(config_path: &Path, force: bool) -> Result<()> {
    let config_path = expand_home(config_path)?;

    if config_path.exists() && !force {
        return Err(format!(
            "config {} already exists; use --force to overwrite it",
            config_path.display()
        )
        .into());
    }

    if let Some(parent) = config_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create config dir {}: {err}", parent.display()))?;
    }

    fs::write(&config_path, documented_config())
        .map_err(|err| format!("failed to write config {}: {err}", config_path.display()))?;
    println!("wrote {}", config_path.display());

    Ok(())
}

pub(crate) fn documented_config() -> &'static str {
    r#"# gfind configuration
#
# Default path: ~/.gfind/config.toml

# Search roots used when --path is not provided.
# Paths beginning with ~/ are expanded relative to HOME.
paths = []

# Include the current working directory in search roots.
# CLI overrides: --cwd, --no-cwd
include_cwd = true

# Print absolute repo paths instead of paths relative to cwd.
# CLI overrides: --absolute, --relative
absolute_paths = false

# Directory regexes excluded from repository traversal.
# Regexes match both the directory name and its path relative to the search root.
# CLI overrides: --exclude-dir, --no-exclude-dirs
exclude_dirs = ["^target$"]

# Default branch search mode.
# Valid values: "current", "local", "remote", "all"
# CLI overrides: --current, --local, --remote, --all
branch_search = "local"

# Default query modes.
# Valid values: "contains", "matches", "exact", "fuzzy", "regex"
# CLI overrides on repos, branch, and stash: --query-mode, --contains, --exact, --fuzzy, --regex
repo_query = "contains"
branch_query = "contains"
stash_query = "contains"

# Branch commit hash output mode.
# Valid values: "full", "short", "none"
# CLI overrides: --commit-hash, --full-commit-hash, --short-commit-hash, --no-commit-hash
branch_hash = "full"

# Print local branch freshness compared to upstream tracking refs.
# CLI overrides: --upstream-status, --no-upstream-status
branch_upstream_status = true

# Prefer checking the live origin remote branch before local tracking refs.
# This may contact the network and is off by default.
# CLI overrides: --origin-status, --no-origin-status
branch_origin_status = false

# Deprecated compatibility setting. Prefer branch_hash = "none" instead.
# branch_hashes = true

# Examples
#
# Search a fixed set of roots instead of cwd by default:
#
# paths = ["~/src", "~/work"]
# include_cwd = false
#
# Prefer absolute repo paths in all output:
#
# absolute_paths = true
#
# Exclude generated dependency and build directories:
#
# exclude_dirs = ["^target$", "^node_modules$", "^vendor/generated$"]
#
# Make branch searches include local and remote refs by default:
#
# branch_search = "all"
#
# Use exact branch queries by default, but fuzzy repo and stash queries:
#
# repo_query = "fuzzy"
# branch_query = "exact"
# stash_query = "fuzzy"
#
# Shorten branch hashes and suppress upstream freshness checks:
#
# branch_hash = "short"
# branch_upstream_status = false
#
# Prefer live origin checks for branch freshness. This may contact the network:
#
# branch_origin_status = true
"#
}

pub(crate) fn load_config(path: &Path, logger: Logger) -> Result<ConfigFile> {
    Ok(load_optional_config(path, logger)?.unwrap_or_default())
}

pub(crate) fn load_optional_config(path: &Path, logger: Logger) -> Result<Option<ConfigFile>> {
    let path = expand_home(path)?;
    logger.log(format!("reading config {}", path.display()));

    match fs::read_to_string(&path) {
        Ok(config) => {
            logger.log(format!("loaded config {}", path.display()));
            Ok(Some(toml::from_str(&config).map_err(|err| {
                format!("failed to parse config {}: {err}", path.display())
            })?))
        }
        Err(err)
            if matches!(
                err.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) =>
        {
            logger.log(format!(
                "failed to read config {}: {err}; using defaults",
                path.display()
            ));
            Ok(None)
        }
        Err(err) => Err(format!("failed to read config {}: {err}", path.display()).into()),
    }
}

pub(crate) fn search_roots(cli: &Cli, config: &ConfigFile, logger: Logger) -> Result<Vec<PathBuf>> {
    let mut roots = Vec::new();

    if cli.paths.is_empty() {
        if let Some(paths) = &config.paths {
            logger.log(format!("using {} configured path(s)", paths.len()));
            roots.extend(paths.iter().cloned());
        }

        let include_cwd = if cli.cwd {
            true
        } else if cli.no_cwd {
            false
        } else {
            config.effective_include_cwd()
        };

        if include_cwd {
            let cwd = env::current_dir().map_err(|err| format!("failed to get cwd: {err}"))?;
            logger.log(format!("including cwd {}", cwd.display()));
            roots.push(cwd);
        } else {
            logger.log("not including cwd");
        }
    } else {
        logger.log(format!("using {} CLI path(s)", cli.paths.len()));
        roots.extend(cli.paths.iter().cloned());

        if cli.cwd {
            let cwd = env::current_dir().map_err(|err| format!("failed to get cwd: {err}"))?;
            logger.log(format!("including cwd {}", cwd.display()));
            roots.push(cwd);
        }
    }

    let roots = dedupe_paths(roots)?;

    for root in &roots {
        logger.log(format!("search root {}", root.display()));
    }

    Ok(roots)
}

pub(crate) fn exclude_dir_patterns(cli: &Cli, config: &ConfigFile) -> Vec<String> {
    if cli.no_exclude_dirs {
        Vec::new()
    } else if !cli.exclude_dirs.is_empty() {
        cli.exclude_dirs.clone()
    } else {
        config.effective_exclude_dirs()
    }
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Result<Vec<PathBuf>> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();

    for path in paths {
        let expanded = expand_home(&path)?;
        let canonical = fs::canonicalize(&expanded)
            .map_err(|err| format!("failed to access search path {}: {err}", expanded.display()))?;

        if seen.insert(canonical.clone()) {
            deduped.push(canonical);
        }
    }

    Ok(deduped)
}

pub(crate) fn expand_home(path: &Path) -> Result<PathBuf> {
    let Some(path_str) = path.to_str() else {
        return Ok(path.to_path_buf());
    };

    if path_str == "~" {
        Ok(home_dir().ok_or_else(|| "failed to determine home directory".to_string())?)
    } else if let Some(rest) = path_str.strip_prefix("~/") {
        let home = home_dir().ok_or_else(|| "failed to determine home directory".to_string())?;
        Ok(home.join(rest))
    } else {
        Ok(path.to_path_buf())
    }
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn resolves_directory_exclusion_precedence() {
        let defaults = Cli::try_parse_from(["gfind", "repos"]).unwrap();
        assert_eq!(
            exclude_dir_patterns(&defaults, &ConfigFile::default()),
            [DEFAULT_EXCLUDE_DIR]
        );

        let config = ConfigFile {
            exclude_dirs: Some(vec!["^vendor$".to_string()]),
            ..ConfigFile::default()
        };
        assert_eq!(exclude_dir_patterns(&defaults, &config), ["^vendor$"]);

        let cli = Cli::try_parse_from(["gfind", "--exclude-dir", "^build$", "repos"]).unwrap();
        assert_eq!(exclude_dir_patterns(&cli, &config), ["^build$"]);

        let disabled = Cli::try_parse_from(["gfind", "--no-exclude-dirs", "repos"]).unwrap();
        assert!(exclude_dir_patterns(&disabled, &config).is_empty());
    }

    #[test]
    fn missing_config_uses_defaults() {
        let config_path = env::temp_dir()
            .join(format!("gfind-config-test-{}", std::process::id()))
            .join("config.toml");
        let config = load_config(&config_path, Logger::new(false)).unwrap();

        assert_eq!(config.paths, None);
    }

    #[test]
    fn unavailable_config_path_uses_defaults() {
        let temp_dir =
            env::temp_dir().join(format!("gfind-legacy-config-test-{}", std::process::id()));
        let legacy_path = temp_dir.join(".gfind");
        fs::create_dir_all(&temp_dir).unwrap();
        fs::write(&legacy_path, "paths = []").unwrap();

        let config = load_config(&legacy_path.join("config.toml"), Logger::new(false)).unwrap();

        assert_eq!(config.paths, None);

        fs::remove_file(legacy_path).unwrap();
        fs::remove_dir(temp_dir).unwrap();
    }
}
