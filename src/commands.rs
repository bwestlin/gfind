use std::collections::BTreeSet;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use regex::RegexSet;

use crate::cli::Cli;
use crate::cli::Commands;
use crate::cli::QueryOptions;
use crate::cli::print_completions;
use crate::config::BranchHash;
use crate::config::BranchSearch;
use crate::config::ConfigFile;
use crate::config::exclude_dir_patterns;
use crate::config::load_config;
use crate::config::run_config_command;
use crate::config::search_roots;
use crate::error::Result;
use crate::git::git_stdout;
use crate::git::git_success;
use crate::logger::Logger;
use crate::query::QueryMatcher;
use crate::query::QueryMode;
use crate::style::OutputStyle;

struct RepoMatch {
    display_path: String,
    display_ranges: Vec<(usize, usize)>,
    absolute_path: Option<MatchedText>,
    remotes: Vec<MatchedText>,
}

#[derive(Debug)]
struct DirectoryExclusions {
    regexes: RegexSet,
}

impl DirectoryExclusions {
    fn new(patterns: Vec<String>) -> Result<Self> {
        let regexes = RegexSet::new(&patterns)
            .map_err(|err| format!("invalid directory exclusion regex: {err}"))?;

        Ok(Self { regexes })
    }

    fn len(&self) -> usize {
        self.regexes.patterns().len()
    }

    fn matches(&self, root: &Path, directory: &Path) -> bool {
        let name_matches = directory
            .file_name()
            .is_some_and(|name| self.regexes.is_match(name.to_string_lossy().as_ref()));
        let relative = directory.strip_prefix(root).unwrap_or(directory);
        let relative = relative
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/");

        name_matches || self.regexes.is_match(&relative)
    }
}

#[derive(Debug)]
struct MatchedText {
    text: String,
    ranges: Vec<(usize, usize)>,
}

#[derive(Debug)]
struct RepoStatus {
    dirty: bool,
    ahead: Option<u32>,
}

#[derive(Debug)]
struct BranchMatches {
    current: bool,
    current_name: Option<String>,
    current_hash: Option<String>,
    current_upstream_status: Option<BranchUpstreamStatus>,
    local: BTreeSet<BranchRef>,
    remote: BTreeSet<BranchRef>,
}

impl BranchMatches {
    fn has_matches(&self, mode: BranchSearch) -> bool {
        match mode {
            BranchSearch::Current => self.current,
            BranchSearch::Local => !self.local.is_empty(),
            BranchSearch::Remote => !self.remote.is_empty(),
            BranchSearch::All => self.current || !self.local.is_empty() || !self.remote.is_empty(),
        }
    }
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
struct BranchRef {
    name: String,
    commit_hash: Option<String>,
    upstream_status: Option<BranchUpstreamStatus>,
}

#[derive(Clone, Copy, Debug)]
struct BranchOutputOptions {
    search: BranchSearch,
    hash: BranchHash,
    upstream_status: bool,
    origin_status: bool,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
enum BranchUpstreamStatus {
    NoUpstream,
    Origin {
        remote: String,
        remote_hash: Option<String>,
        freshness: BranchFreshness,
    },
    Tracking {
        upstream: String,
        upstream_hash: Option<String>,
        ahead: u32,
        behind: u32,
    },
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
enum BranchFreshness {
    UpToDate,
    Ahead(u32),
    Behind(u32),
    Diverged { ahead: u32, behind: u32 },
    Different,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct StashEntry {
    reference: String,
    subject: String,
}

struct DisplayOptions {
    absolute_paths: bool,
    cwd: PathBuf,
    style: OutputStyle,
}

impl DisplayOptions {
    fn new(cli: &Cli, config: &ConfigFile) -> Result<Self> {
        let absolute_paths = if cli.absolute {
            true
        } else if cli.relative {
            false
        } else {
            config.effective_absolute_paths()
        };
        let cwd = env::current_dir()
            .map_err(|err| format!("failed to get cwd: {err}"))
            .and_then(|cwd| {
                fs::canonicalize(&cwd)
                    .map_err(|err| format!("failed to access cwd {}: {err}", cwd.display()))
            })?;

        Ok(Self {
            absolute_paths,
            cwd,
            style: OutputStyle::auto(),
        })
    }

    fn repo(&self, repo: &Path) -> String {
        if self.absolute_paths {
            repo.display().to_string()
        } else {
            relative_path(&self.cwd, repo).display().to_string()
        }
    }
}

pub(crate) fn run(cli: Cli) -> Result<()> {
    let logger = Logger::new(cli.verbose);

    if let Commands::Completions { shell } = cli.command {
        print_completions(shell);
        return Ok(());
    }

    if let Commands::Config { command } = &cli.command {
        return run_config_command(command, &cli.config);
    }

    let config = load_config(&cli.config, logger)?;
    let display = DisplayOptions::new(&cli, &config)?;
    let roots = search_roots(&cli, &config, logger)?;
    let exclusions = DirectoryExclusions::new(exclude_dir_patterns(&cli, &config))?;
    logger.log(format!("using {} directory exclusion(s)", exclusions.len()));
    let repos = discover_repos(&roots, &exclusions, logger)?;

    logger.log(format!("discovered {} repo(s)", repos.len()));

    match cli.command {
        Commands::Completions { .. } => {
            unreachable!("completion commands are handled before repo discovery")
        }
        Commands::Config { .. } => {
            unreachable!("config commands are handled before repo discovery")
        }
        Commands::Repos {
            query,
            query_options,
        } => {
            let matcher = query
                .as_deref()
                .map(|query| {
                    let mode = query_mode(&query_options, config.effective_repo_query());
                    QueryMatcher::new(query, mode)
                })
                .transpose()?;
            print_repos(&repos, matcher.as_ref(), &display, logger)
        }
        Commands::Branch {
            query,
            query_options,
            commit_hash,
            short_commit_hash,
            no_commit_hash,
            upstream_status,
            no_upstream_status,
            origin_status,
            no_origin_status,
            current,
            local,
            remote,
            all,
        } => {
            let search = branch_search_mode(current, local, remote, all, &config)?;
            let hash = branch_hash_mode(commit_hash, short_commit_hash, no_commit_hash, &config);
            let upstream_status =
                branch_upstream_status(upstream_status, no_upstream_status, &config);
            let origin_status = branch_origin_status(origin_status, no_origin_status, &config);
            let query_mode = query_mode(&query_options, config.effective_branch_query());
            let matcher = QueryMatcher::new(&query, query_mode)?;
            let branch_options = BranchOutputOptions {
                search,
                hash,
                upstream_status,
                origin_status,
            };
            print_branch_matches(&repos, &matcher, branch_options, &display, logger)
        }
        Commands::Commit { rev, reachable } => {
            print_commit_matches(&repos, &rev, reachable, &display, logger)
        }
        Commands::Status {
            dirty_only,
            unpushed_only,
        } => print_status_matches(&repos, dirty_only, unpushed_only, &display, logger),
        Commands::Stash {
            query,
            query_options,
            print,
            patch,
            stash,
        } => {
            let matcher = query
                .as_deref()
                .map(|query| {
                    let mode = query_mode(&query_options, config.effective_stash_query());
                    QueryMatcher::new(query, mode)
                })
                .transpose()?;
            print_stash_matches(
                &repos,
                matcher.as_ref(),
                print || patch,
                patch,
                stash.as_deref(),
                &display,
                logger,
            )
        }
    }
}

fn branch_hash_mode(
    commit_hash: bool,
    short_commit_hash: bool,
    no_commit_hash: bool,
    config: &ConfigFile,
) -> BranchHash {
    if commit_hash {
        BranchHash::Full
    } else if short_commit_hash {
        BranchHash::Short
    } else if no_commit_hash {
        BranchHash::None
    } else {
        config.effective_branch_hash()
    }
}

fn branch_upstream_status(
    upstream_status: bool,
    no_upstream_status: bool,
    config: &ConfigFile,
) -> bool {
    if upstream_status {
        true
    } else if no_upstream_status {
        false
    } else {
        config.effective_branch_upstream_status()
    }
}

fn branch_origin_status(origin_status: bool, no_origin_status: bool, config: &ConfigFile) -> bool {
    if origin_status {
        true
    } else if no_origin_status {
        false
    } else {
        config.effective_branch_origin_status()
    }
}

fn query_mode(options: &QueryOptions, configured: QueryMode) -> QueryMode {
    if let Some(mode) = options.query_mode {
        mode
    } else if options.contains {
        QueryMode::Contains
    } else if options.exact {
        QueryMode::Exact
    } else if options.fuzzy {
        QueryMode::Fuzzy
    } else if options.regex {
        QueryMode::Regex
    } else {
        configured
    }
}

fn branch_search_mode(
    current: bool,
    local: bool,
    remote: bool,
    all: bool,
    config: &ConfigFile,
) -> Result<BranchSearch> {
    let selected = [current, local, remote, all]
        .into_iter()
        .filter(|selected| *selected)
        .count();

    if selected > 1 {
        return Err("use only one of --current, --local, --remote, or --all".into());
    }

    if current {
        Ok(BranchSearch::Current)
    } else if local {
        Ok(BranchSearch::Local)
    } else if remote {
        Ok(BranchSearch::Remote)
    } else if all {
        Ok(BranchSearch::All)
    } else {
        Ok(config.effective_branch_search())
    }
}

fn discover_repos(
    roots: &[PathBuf],
    exclusions: &DirectoryExclusions,
    logger: Logger,
) -> Result<Vec<PathBuf>> {
    let mut seen = HashSet::new();
    let mut repos = Vec::new();

    for root in roots {
        logger.log(format!("scanning {}", root.display()));

        if let Some(repo) = repo_toplevel(root, logger)? {
            push_repo(&mut repos, &mut seen, repo, logger)?;
        }

        let mut stack = vec![root.clone()];

        while let Some(path) = stack.pop() {
            let entries = match fs::read_dir(&path) {
                Ok(entries) => entries,
                Err(err) => {
                    logger.log(format!(
                        "skipping unreadable directory {}: {err}",
                        path.display()
                    ));
                    continue;
                }
            };

            for entry in entries.flatten() {
                let entry_path = entry.path();
                let file_name = entry.file_name();

                if file_name == ".git" {
                    continue;
                }

                let Ok(file_type) = entry.file_type() else {
                    continue;
                };

                if !file_type.is_dir() || file_type.is_symlink() {
                    continue;
                }

                if exclusions.matches(root, &entry_path) {
                    logger.log(format!(
                        "skipping excluded directory {}",
                        entry_path.display()
                    ));
                    continue;
                }

                if entry_path.join(".git").exists()
                    && let Some(repo) = repo_toplevel(&entry_path, logger)?
                {
                    push_repo(&mut repos, &mut seen, repo, logger)?;
                }

                stack.push(entry_path);
            }
        }
    }

    repos.sort();
    Ok(repos)
}

fn push_repo(
    repos: &mut Vec<PathBuf>,
    seen: &mut HashSet<PathBuf>,
    repo: PathBuf,
    logger: Logger,
) -> Result<()> {
    let canonical = fs::canonicalize(&repo)
        .map_err(|err| format!("failed to access repo {}: {err}", repo.display()))?;

    if seen.insert(canonical.clone()) {
        logger.log(format!("found repo {}", canonical.display()));
        repos.push(canonical);
    } else {
        logger.log(format!("already saw repo {}", canonical.display()));
    }

    Ok(())
}

fn repo_toplevel(path: &Path, logger: Logger) -> Result<Option<PathBuf>> {
    logger.log(format!("checking git toplevel {}", path.display()));

    let output = Command::new("git")
        .args(["-C"])
        .arg(path)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|err| format!("failed to run git: {err}"))?;

    if !output.status.success() {
        logger.log(format!("not a git repo {}", path.display()));
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let Some(line) = stdout.lines().next() else {
        return Ok(None);
    };

    Ok(Some(PathBuf::from(line)))
}

fn print_repos(
    repos: &[PathBuf],
    matcher: Option<&QueryMatcher>,
    display: &DisplayOptions,
    logger: Logger,
) -> Result<()> {
    for repo in repos {
        logger.log(format!("checking repo {}", repo.display()));

        let Some(repo_match) = repo_match(repo, matcher, display, logger)? else {
            continue;
        };

        println!(
            "{}",
            display
                .style
                .highlight_repo(&repo_match.display_path, &repo_match.display_ranges)
        );

        if let Some(absolute_path) = repo_match.absolute_path {
            println!(
                "  {}: {}",
                display.style.label("absolute"),
                display
                    .style
                    .highlight_repo(&absolute_path.text, &absolute_path.ranges)
            );
        }

        for remote in repo_match.remotes {
            println!(
                "  {}: {}",
                display.style.label("remote"),
                display.style.highlight_plain(&remote.text, &remote.ranges)
            );
        }
    }

    Ok(())
}

fn repo_match(
    repo: &Path,
    matcher: Option<&QueryMatcher>,
    display: &DisplayOptions,
    logger: Logger,
) -> Result<Option<RepoMatch>> {
    let display_path = display.repo(repo);
    let Some(matcher) = matcher else {
        return Ok(Some(RepoMatch {
            display_path,
            display_ranges: Vec::new(),
            absolute_path: None,
            remotes: Vec::new(),
        }));
    };

    let display_ranges = matcher.match_ranges(&display_path);
    let absolute_path = repo.to_string_lossy().into_owned();
    let absolute_path = matched_text(absolute_path, matcher).filter(|_| display_ranges.is_empty());
    let path_matches = !display_ranges.is_empty() || absolute_path.is_some();

    let remotes = if path_matches {
        Vec::new()
    } else {
        matching_remote_lines(repo, matcher, logger)?
    };

    if path_matches || !remotes.is_empty() {
        return Ok(Some(RepoMatch {
            display_path,
            display_ranges,
            absolute_path,
            remotes,
        }));
    }

    Ok(None)
}

fn matched_text(text: impl Into<String>, matcher: &QueryMatcher) -> Option<MatchedText> {
    let text = text.into();
    let ranges = matcher.match_ranges(&text);

    (!ranges.is_empty()).then_some(MatchedText { text, ranges })
}

fn matching_remote_lines(
    repo: &Path,
    matcher: &QueryMatcher,
    logger: Logger,
) -> Result<Vec<MatchedText>> {
    let Some(remotes) = git_stdout(repo, &["remote", "-v"], logger)? else {
        return Ok(Vec::new());
    };

    Ok(remotes
        .lines()
        .filter_map(|line| matched_text(line, matcher))
        .collect())
}

fn print_branch_matches(
    repos: &[PathBuf],
    matcher: &QueryMatcher,
    options: BranchOutputOptions,
    display: &DisplayOptions,
    logger: Logger,
) -> Result<()> {
    for repo in repos {
        logger.log(format!("checking branch in {}", repo.display()));
        let matches = matching_branches(
            repo,
            matcher,
            options.search,
            options.hash,
            options.upstream_status,
            options.origin_status,
            logger,
        )?;

        if !matches.has_matches(options.search) {
            continue;
        }

        println!("{}", display.style.repo(display.repo(repo)));

        match options.search {
            BranchSearch::Current => print_current_branch(&matches, matcher, &display.style),
            BranchSearch::Local => {
                for branch in &matches.local {
                    let current = matches.current_name.as_deref() == Some(branch.name.as_str());
                    print_branch_ref("local", branch, current, matcher, &display.style);
                }
            }
            BranchSearch::Remote => {
                for branch in matches.remote {
                    print_branch_ref("remote", &branch, false, matcher, &display.style);
                }
            }
            BranchSearch::All => {
                if matches.current
                    && !matches
                        .local
                        .iter()
                        .any(|branch| matches.current_name.as_deref() == Some(branch.name.as_str()))
                {
                    print_current_branch(&matches, matcher, &display.style);
                }

                for branch in &matches.local {
                    let current = matches.current_name.as_deref() == Some(branch.name.as_str());
                    print_branch_ref("local", branch, current, matcher, &display.style);
                }

                for branch in matches.remote {
                    print_branch_ref("remote", &branch, false, matcher, &display.style);
                }
            }
        }
    }

    Ok(())
}

fn print_current_branch(matches: &BranchMatches, matcher: &QueryMatcher, style: &OutputStyle) {
    if let Some(branch) = matches.current_name.as_deref() {
        println!(
            "  {}: {}",
            style.label("current"),
            matcher.highlight_branch_name(branch, style)
        );
    }

    if let Some(commit_hash) = &matches.current_hash {
        println!(
            "    {}: {}",
            style.label("commit"),
            style.commit(commit_hash)
        );
    }

    if let Some(upstream_status) = &matches.current_upstream_status {
        println!(
            "    {}: {}",
            style.label("status"),
            style.status(format_branch_upstream_status(upstream_status))
        );
    }
}

fn print_branch_ref(
    kind: &str,
    branch: &BranchRef,
    current: bool,
    matcher: &QueryMatcher,
    style: &OutputStyle,
) {
    println!(
        "  {}: {}",
        style.label(kind),
        matcher.highlight_branch_name(&branch.name, style)
    );

    if let Some(commit_hash) = &branch.commit_hash {
        println!(
            "    {}: {}",
            style.label("commit"),
            style.commit(commit_hash)
        );
    }

    if let Some(upstream_status) = &branch.upstream_status {
        println!(
            "    {}: {}",
            style.label("status"),
            style.status(format_branch_upstream_status(upstream_status))
        );
    }

    if current {
        println!("    {}: {}", style.label("current"), style.current("yes"));
    }
}

fn format_branch_upstream_status(status: &BranchUpstreamStatus) -> String {
    match status {
        BranchUpstreamStatus::NoUpstream => "no upstream".to_string(),
        BranchUpstreamStatus::Origin {
            remote,
            remote_hash,
            freshness,
        } => format!(
            "{} {}",
            format_branch_freshness(freshness),
            format_upstream(remote, remote_hash)
        ),
        BranchUpstreamStatus::Tracking {
            upstream,
            upstream_hash,
            ahead: 0,
            behind: 0,
        } => format!(
            "up-to-date with {}",
            format_upstream(upstream, upstream_hash)
        ),
        BranchUpstreamStatus::Tracking {
            upstream,
            upstream_hash,
            ahead,
            behind: 0,
        } => format!(
            "ahead {ahead} of {}",
            format_upstream(upstream, upstream_hash)
        ),
        BranchUpstreamStatus::Tracking {
            upstream,
            upstream_hash,
            ahead: 0,
            behind,
        } => format!(
            "behind {behind} from {}",
            format_upstream(upstream, upstream_hash)
        ),
        BranchUpstreamStatus::Tracking {
            upstream,
            upstream_hash,
            ahead,
            behind,
        } => format!(
            "ahead {ahead}, behind {behind} vs {}",
            format_upstream(upstream, upstream_hash)
        ),
    }
}

fn format_branch_freshness(freshness: &BranchFreshness) -> String {
    match freshness {
        BranchFreshness::UpToDate => "up-to-date with".to_string(),
        BranchFreshness::Ahead(ahead) => format!("ahead {ahead} of"),
        BranchFreshness::Behind(behind) => format!("behind {behind} from"),
        BranchFreshness::Diverged { ahead, behind } => {
            format!("ahead {ahead}, behind {behind} vs")
        }
        BranchFreshness::Different => "differs from".to_string(),
    }
}

fn format_upstream(upstream: &str, upstream_hash: &Option<String>) -> String {
    upstream_hash.as_ref().map_or_else(
        || upstream.to_string(),
        |hash| format!("{upstream} @ {hash}"),
    )
}

fn branch_status_for(
    repo: &Path,
    branch: &str,
    hash: BranchHash,
    prefer_origin_status: bool,
    logger: Logger,
) -> Result<Option<BranchUpstreamStatus>> {
    if prefer_origin_status
        && let Some(status) = branch_origin_status_for(repo, branch, hash, logger)?
    {
        return Ok(Some(status));
    }

    branch_upstream_status_for(repo, branch, hash, logger)
}

fn current_branch(repo: &Path, logger: Logger) -> Result<Option<String>> {
    Ok(git_stdout(repo, &["branch", "--show-current"], logger)?
        .map(|branch| branch.trim().to_string())
        .filter(|branch| !branch.is_empty()))
}

fn matching_branches(
    repo: &Path,
    matcher: &QueryMatcher,
    search: BranchSearch,
    hash: BranchHash,
    include_upstream_status: bool,
    prefer_origin_status: bool,
    logger: Logger,
) -> Result<BranchMatches> {
    let current_name = current_branch(repo, logger)?;
    let current = current_name
        .as_deref()
        .is_some_and(|branch| matcher.is_match(branch));
    let current_hash = if current && hash != BranchHash::None {
        current_commit_hash(repo, hash, logger)?
    } else {
        None
    };
    let current_upstream_status = if current && include_upstream_status {
        let branch = current_name
            .as_deref()
            .ok_or_else(|| format!("failed to read current branch for {}", repo.display()))?;
        branch_status_for(repo, branch, hash, prefer_origin_status, logger)?
    } else {
        None
    };
    let mut local = BTreeSet::new();
    let mut remote = BTreeSet::new();

    if matches!(search, BranchSearch::Local | BranchSearch::All) {
        local = matching_branch_refs(
            repo,
            matcher,
            BranchRefKind::Local,
            hash,
            include_upstream_status,
            prefer_origin_status,
            logger,
        )?;
    }

    if matches!(search, BranchSearch::Remote | BranchSearch::All) {
        remote = matching_branch_refs(
            repo,
            matcher,
            BranchRefKind::Remote,
            hash,
            false,
            false,
            logger,
        )?;
    }

    Ok(BranchMatches {
        current,
        current_name,
        current_hash,
        current_upstream_status,
        local,
        remote,
    })
}

fn current_commit_hash(repo: &Path, hash: BranchHash, logger: Logger) -> Result<Option<String>> {
    branch_commit_hash(repo, "HEAD", hash, logger)
}

fn branch_commit_hash(
    repo: &Path,
    rev: &str,
    hash: BranchHash,
    logger: Logger,
) -> Result<Option<String>> {
    let rev_parse_arg = match hash {
        BranchHash::None => return Ok(None),
        BranchHash::Short => "--short",
        BranchHash::Full => "--verify",
    };

    Ok(
        git_stdout(repo, &["rev-parse", rev_parse_arg, rev], logger)?
            .map(|hash| hash.trim().to_string())
            .filter(|hash| !hash.is_empty()),
    )
}

fn branch_origin_status_for(
    repo: &Path,
    branch: &str,
    hash: BranchHash,
    logger: Logger,
) -> Result<Option<BranchUpstreamStatus>> {
    let branch_ref = format!("refs/heads/{branch}");
    let Some(output) = git_stdout(repo, &["ls-remote", "origin", &branch_ref], logger)? else {
        return Ok(None);
    };
    let Some(remote_hash_full) = parse_ls_remote_hash(&output, &branch_ref) else {
        return Ok(None);
    };
    let Some(local_hash_full) = branch_commit_hash(repo, &branch_ref, BranchHash::Full, logger)?
    else {
        return Ok(None);
    };
    let remote_hash = display_hash(&remote_hash_full, hash);
    let freshness = branch_freshness(
        repo,
        &branch_ref,
        &local_hash_full,
        &remote_hash_full,
        logger,
    )?;

    Ok(Some(BranchUpstreamStatus::Origin {
        remote: format!("origin/{branch}"),
        remote_hash,
        freshness,
    }))
}

fn parse_ls_remote_hash(output: &str, expected_ref: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let (hash, reference) = line.split_once('\t')?;
        (reference == expected_ref).then(|| hash.to_string())
    })
}

fn display_hash(hash: &str, mode: BranchHash) -> Option<String> {
    match mode {
        BranchHash::None => None,
        BranchHash::Short => Some(hash.chars().take(7).collect()),
        BranchHash::Full => Some(hash.to_string()),
    }
}

fn branch_freshness(
    repo: &Path,
    local_ref: &str,
    local_hash_full: &str,
    remote_hash_full: &str,
    logger: Logger,
) -> Result<BranchFreshness> {
    if local_hash_full == remote_hash_full {
        return Ok(BranchFreshness::UpToDate);
    }

    let range = format!("{local_ref}...{remote_hash_full}");
    let Some(counts) = git_stdout(
        repo,
        &["rev-list", "--left-right", "--count", &range],
        logger,
    )?
    else {
        return Ok(BranchFreshness::Different);
    };
    let Some((ahead, behind)) = parse_ahead_behind(&counts) else {
        return Ok(BranchFreshness::Different);
    };

    Ok(match (ahead, behind) {
        (0, 0) => BranchFreshness::UpToDate,
        (ahead, 0) => BranchFreshness::Ahead(ahead),
        (0, behind) => BranchFreshness::Behind(behind),
        (ahead, behind) => BranchFreshness::Diverged { ahead, behind },
    })
}

fn branch_upstream_status_for(
    repo: &Path,
    branch: &str,
    hash: BranchHash,
    logger: Logger,
) -> Result<Option<BranchUpstreamStatus>> {
    let upstream_ref = format!("{branch}@{{upstream}}");
    let Some(upstream) = git_stdout(repo, &["rev-parse", "--abbrev-ref", &upstream_ref], logger)?
    else {
        return Ok(Some(BranchUpstreamStatus::NoUpstream));
    };
    let upstream = upstream.trim();

    if upstream.is_empty() {
        return Ok(Some(BranchUpstreamStatus::NoUpstream));
    }

    let upstream_hash = if hash == BranchHash::None {
        None
    } else {
        branch_commit_hash(repo, upstream, hash, logger)?
    };

    let range = format!("{branch}...{upstream}");
    let Some(counts) = git_stdout(
        repo,
        &["rev-list", "--left-right", "--count", &range],
        logger,
    )?
    else {
        return Ok(Some(BranchUpstreamStatus::NoUpstream));
    };
    let (ahead, behind) = parse_ahead_behind(&counts).ok_or_else(|| {
        format!(
            "failed to parse upstream status for {} branch {branch}",
            repo.display()
        )
    })?;

    Ok(Some(BranchUpstreamStatus::Tracking {
        upstream: upstream.to_string(),
        upstream_hash,
        ahead,
        behind,
    }))
}

fn parse_ahead_behind(output: &str) -> Option<(u32, u32)> {
    let mut parts = output.split_whitespace();
    let ahead = parts.next()?.parse::<u32>().ok()?;
    let behind = parts.next()?.parse::<u32>().ok()?;

    Some((ahead, behind))
}

#[derive(Clone, Copy)]
enum BranchRefKind {
    Local,
    Remote,
}

fn matching_branch_refs(
    repo: &Path,
    matcher: &QueryMatcher,
    kind: BranchRefKind,
    hash: BranchHash,
    include_upstream_status: bool,
    prefer_origin_status: bool,
    logger: Logger,
) -> Result<BTreeSet<BranchRef>> {
    let ref_root = match kind {
        BranchRefKind::Local => "refs/heads",
        BranchRefKind::Remote => "refs/remotes",
    };
    let format = match hash {
        BranchHash::None => "%(refname)",
        BranchHash::Short => "%(refname)%09%(objectname:short)",
        BranchHash::Full => "%(refname)%09%(objectname)",
    };
    let Some(output) = git_stdout(
        repo,
        &["for-each-ref", &format!("--format={format}"), ref_root],
        logger,
    )?
    else {
        return Ok(BTreeSet::new());
    };

    let mut refs: BTreeSet<BranchRef> = output
        .lines()
        .filter_map(|reference| printable_branch_ref(reference, matcher, kind))
        .collect();

    if include_upstream_status && matches!(kind, BranchRefKind::Local) {
        refs = refs
            .into_iter()
            .map(|mut branch| {
                branch.upstream_status =
                    branch_status_for(repo, &branch.name, hash, prefer_origin_status, logger)?;
                Ok(branch)
            })
            .collect::<Result<BTreeSet<_>>>()?;
    }

    Ok(refs)
}

fn printable_branch_ref(
    reference: &str,
    matcher: &QueryMatcher,
    kind: BranchRefKind,
) -> Option<BranchRef> {
    let (reference, commit_hash) = reference
        .split_once('\t')
        .map_or((reference, None), |(reference, hash)| {
            (reference, Some(hash.to_string()))
        });

    match kind {
        BranchRefKind::Local => reference
            .strip_prefix("refs/heads/")
            .filter(|local| matcher.is_match(local))
            .map(|local| BranchRef {
                name: local.to_string(),
                commit_hash,
                upstream_status: None,
            }),
        BranchRefKind::Remote => {
            let remote = reference.strip_prefix("refs/remotes/")?;
            let (_, remote_branch) = remote.split_once('/')?;
            (matcher.is_match(remote_branch) || matcher.is_match(remote)).then(|| BranchRef {
                name: remote.to_string(),
                commit_hash,
                upstream_status: None,
            })
        }
    }
}

fn print_commit_matches(
    repos: &[PathBuf],
    rev: &str,
    reachable: bool,
    display: &DisplayOptions,
    logger: Logger,
) -> Result<()> {
    for repo in repos {
        logger.log(format!("checking commit in {}", repo.display()));

        let matches = if reachable {
            git_success(repo, &["merge-base", "--is-ancestor", rev, "HEAD"], logger)?
        } else {
            let commit_rev = format!("{rev}^{{commit}}");
            git_success(repo, &["cat-file", "-e", &commit_rev], logger)?
        };

        if matches {
            println!("{}", display.style.repo(display.repo(repo)));
            println!(
                "  {}: {}",
                display.style.label("commit"),
                display.style.highlight_commit(rev, &[(0, rev.len())])
            );
        }
    }

    Ok(())
}

fn print_status_matches(
    repos: &[PathBuf],
    dirty_only: bool,
    unpushed_only: bool,
    display: &DisplayOptions,
    logger: Logger,
) -> Result<()> {
    for repo in repos {
        logger.log(format!("checking status in {}", repo.display()));

        let status = repo_status(repo, logger)?;
        let dirty_matches = status.dirty && !unpushed_only;
        let unpushed_matches = status.ahead.is_some_and(|ahead| ahead > 0) && !dirty_only;

        if dirty_matches || unpushed_matches {
            let mut reasons = Vec::new();

            if dirty_matches {
                reasons.push("dirty".to_string());
            }

            if unpushed_matches {
                let ahead = status.ahead.unwrap_or(0);
                reasons.push(format!("ahead {ahead}"));
            }

            let reasons = reasons
                .into_iter()
                .map(|reason| display.style.reason(reason))
                .collect::<Vec<_>>()
                .join(", ");
            println!("{} ({})", display.style.repo(display.repo(repo)), reasons);
        }
    }

    Ok(())
}

fn repo_status(repo: &Path, logger: Logger) -> Result<RepoStatus> {
    let dirty = git_stdout(repo, &["status", "--porcelain=v1"], logger)?
        .is_some_and(|status| !status.trim().is_empty());
    let ahead = upstream_ahead_count(repo, logger)?;

    Ok(RepoStatus { dirty, ahead })
}

fn upstream_ahead_count(repo: &Path, logger: Logger) -> Result<Option<u32>> {
    let Some(upstream) = git_stdout(
        repo,
        &[
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            "@{upstream}",
        ],
        logger,
    )?
    else {
        logger.log(format!("no upstream for {}", repo.display()));
        return Ok(None);
    };

    let range = format!("{}..HEAD", upstream.trim());
    let Some(count) = git_stdout(repo, &["rev-list", "--count", &range], logger)? else {
        return Ok(None);
    };

    Ok(count
        .trim()
        .parse::<u32>()
        .map(Some)
        .map_err(|err| format!("failed to parse ahead count for {}: {err}", repo.display()))?)
}

fn print_stash_matches(
    repos: &[PathBuf],
    matcher: Option<&QueryMatcher>,
    print: bool,
    patch: bool,
    stash: Option<&str>,
    display: &DisplayOptions,
    logger: Logger,
) -> Result<()> {
    for repo in repos {
        logger.log(format!("checking stashes in {}", repo.display()));

        let entries = matching_stashes(repo, matcher, stash, logger)?;

        if entries.is_empty() {
            continue;
        }

        println!("{}", display.style.repo(display.repo(repo)));

        for entry in entries {
            let reference = matcher.map_or_else(
                || display.style.stash_ref(&entry.reference),
                |matcher| {
                    display.style.highlight_stash_ref(
                        &entry.reference,
                        &matcher.match_ranges(&entry.reference),
                    )
                },
            );
            let subject = matcher.map_or_else(
                || entry.subject.clone(),
                |matcher| {
                    display
                        .style
                        .highlight_plain(&entry.subject, &matcher.match_ranges(&entry.subject))
                },
            );

            println!("  {}: {}", reference, subject);

            if print {
                let mode = if patch { "--patch" } else { "--stat" };

                if let Some(output) =
                    git_stdout(repo, &["stash", "show", mode, &entry.reference], logger)?
                {
                    for line in output.lines() {
                        println!("    {line}");
                    }
                }
            }
        }
    }

    Ok(())
}

fn matching_stashes(
    repo: &Path,
    matcher: Option<&QueryMatcher>,
    stash: Option<&str>,
    logger: Logger,
) -> Result<Vec<StashEntry>> {
    let Some(output) = git_stdout(repo, &["stash", "list", "--format=%gd%x09%gs"], logger)? else {
        return Ok(Vec::new());
    };

    let stash = stash.map(normalize_stash_ref);

    Ok(output
        .lines()
        .filter_map(parse_stash_entry)
        .filter(|entry| {
            stash.as_ref().is_none_or(|stash| &entry.reference == stash)
                && matcher.is_none_or(|matcher| stash_matches(entry, matcher))
        })
        .collect())
}

fn parse_stash_entry(line: &str) -> Option<StashEntry> {
    let (reference, subject) = line.split_once('\t')?;

    Some(StashEntry {
        reference: reference.to_string(),
        subject: subject.to_string(),
    })
}

fn normalize_stash_ref(stash: &str) -> String {
    if stash.chars().all(|char| char.is_ascii_digit()) {
        format!("stash@{{{stash}}}")
    } else {
        stash.to_string()
    }
}

fn stash_matches(entry: &StashEntry, matcher: &QueryMatcher) -> bool {
    matcher.is_match(&entry.reference) || matcher.is_match(&entry.subject)
}

fn relative_path(base: &Path, path: &Path) -> PathBuf {
    if base == path {
        return PathBuf::from(".");
    }

    let base_components: Vec<_> = base.components().collect();
    let path_components: Vec<_> = path.components().collect();
    let common_len = base_components
        .iter()
        .zip(path_components.iter())
        .take_while(|(base, path)| base == path)
        .count();

    if common_len == 0 {
        return path.to_path_buf();
    }

    let mut relative = PathBuf::new();

    for _ in &base_components[common_len..] {
        relative.push("..");
    }

    for component in &path_components[common_len..] {
        relative.push(component.as_os_str());
    }

    if relative.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        relative
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::documented_config;

    #[test]
    fn normalizes_numeric_stash_refs() {
        assert_eq!(normalize_stash_ref("0"), "stash@{0}");
        assert_eq!(normalize_stash_ref("stash@{2}"), "stash@{2}");
    }

    #[test]
    fn parses_stash_entries() {
        let entry = parse_stash_entry("stash@{0}\tWIP on main: message").unwrap();

        assert_eq!(entry.reference, "stash@{0}");
        assert_eq!(entry.subject, "WIP on main: message");
    }

    #[test]
    fn matches_local_branches_by_exact_name() {
        let matcher = QueryMatcher::new("feature/search", QueryMode::Exact).unwrap();
        let non_matcher = QueryMatcher::new("search", QueryMode::Exact).unwrap();

        assert_eq!(
            printable_branch_ref("refs/heads/feature/search", &matcher, BranchRefKind::Local,),
            Some(BranchRef {
                name: "feature/search".to_string(),
                commit_hash: None,
                upstream_status: None,
            })
        );
        assert_eq!(
            printable_branch_ref(
                "refs/heads/feature/search",
                &non_matcher,
                BranchRefKind::Local,
            ),
            None
        );
    }

    #[test]
    fn parses_branch_refs_with_hashes() {
        let feature_matcher = QueryMatcher::new("feature/search", QueryMode::Exact).unwrap();
        let main_matcher = QueryMatcher::new("main", QueryMode::Exact).unwrap();

        assert_eq!(
            printable_branch_ref(
                "refs/heads/feature/search\tabc1234",
                &feature_matcher,
                BranchRefKind::Local,
            ),
            Some(BranchRef {
                name: "feature/search".to_string(),
                commit_hash: Some("abc1234".to_string()),
                upstream_status: None,
            })
        );
        assert_eq!(
            printable_branch_ref(
                "refs/remotes/origin/main\tdef5678",
                &main_matcher,
                BranchRefKind::Remote,
            ),
            Some(BranchRef {
                name: "origin/main".to_string(),
                commit_hash: Some("def5678".to_string()),
                upstream_status: None,
            })
        );
    }

    #[test]
    fn matches_remote_branches_without_remote_name() {
        let feature_matcher = QueryMatcher::new("feature/search", QueryMode::Exact).unwrap();
        let main_matcher = QueryMatcher::new("main", QueryMode::Exact).unwrap();

        assert_eq!(
            printable_branch_ref(
                "refs/remotes/origin/feature/search",
                &feature_matcher,
                BranchRefKind::Remote,
            ),
            Some(BranchRef {
                name: "origin/feature/search".to_string(),
                commit_hash: None,
                upstream_status: None,
            })
        );
        assert_eq!(
            printable_branch_ref(
                "refs/remotes/upstream/main",
                &main_matcher,
                BranchRefKind::Remote,
            ),
            Some(BranchRef {
                name: "upstream/main".to_string(),
                commit_hash: None,
                upstream_status: None,
            })
        );
    }

    #[test]
    fn formats_relative_paths_from_cwd() {
        assert_eq!(
            relative_path(Path::new("/repo"), Path::new("/repo/project")),
            PathBuf::from("project")
        );
        assert_eq!(
            relative_path(Path::new("/repo/project"), Path::new("/repo")),
            PathBuf::from("..")
        );
        assert_eq!(
            relative_path(Path::new("/repo/project"), Path::new("/repo/project")),
            PathBuf::from(".")
        );
    }

    #[test]
    fn branch_search_cli_overrides_config() {
        let config = ConfigFile {
            branch_search: Some(BranchSearch::All),
            ..ConfigFile::default()
        };

        assert_eq!(
            branch_search_mode(false, true, false, false, &config).unwrap(),
            BranchSearch::Local
        );
        assert_eq!(
            branch_search_mode(false, false, false, false, &config).unwrap(),
            BranchSearch::All
        );
        assert!(branch_search_mode(true, true, false, false, &config).is_err());
    }

    #[test]
    fn branch_hash_cli_overrides_config() {
        let config = ConfigFile {
            branch_hash: Some(BranchHash::Short),
            ..ConfigFile::default()
        };

        assert_eq!(
            branch_hash_mode(true, false, false, &config),
            BranchHash::Full
        );
        assert_eq!(
            branch_hash_mode(false, true, false, &config),
            BranchHash::Short
        );
        assert_eq!(
            branch_hash_mode(false, false, true, &config),
            BranchHash::None
        );
        assert_eq!(
            branch_hash_mode(false, false, false, &config),
            BranchHash::Short
        );
        assert_eq!(
            branch_hash_mode(false, false, false, &ConfigFile::default()),
            BranchHash::Full
        );
    }

    #[test]
    fn branch_hash_mode_keeps_branch_hashes_compatibility() {
        let disabled = ConfigFile {
            branch_hashes: Some(false),
            ..ConfigFile::default()
        };
        let enabled = ConfigFile {
            branch_hashes: Some(true),
            ..ConfigFile::default()
        };

        assert_eq!(
            branch_hash_mode(false, false, false, &disabled),
            BranchHash::None
        );
        assert_eq!(
            branch_hash_mode(false, false, false, &enabled),
            BranchHash::Full
        );
    }

    #[test]
    fn branch_upstream_status_cli_overrides_config() {
        let config = ConfigFile {
            branch_upstream_status: Some(false),
            ..ConfigFile::default()
        };

        assert!(branch_upstream_status(true, false, &config));
        assert!(!branch_upstream_status(false, true, &config));
        assert!(!branch_upstream_status(false, false, &config));
        assert!(branch_upstream_status(false, false, &ConfigFile::default()));
    }

    #[test]
    fn branch_origin_status_cli_overrides_config() {
        let config = ConfigFile {
            branch_origin_status: Some(false),
            ..ConfigFile::default()
        };

        assert!(branch_origin_status(true, false, &config));
        assert!(!branch_origin_status(false, true, &config));
        assert!(!branch_origin_status(false, false, &config));
        assert!(!branch_origin_status(false, false, &ConfigFile::default()));
    }

    #[test]
    fn formats_branch_upstream_status() {
        assert_eq!(
            format_branch_upstream_status(&BranchUpstreamStatus::NoUpstream),
            "no upstream"
        );
        assert_eq!(
            format_branch_upstream_status(&BranchUpstreamStatus::Origin {
                remote: "origin/main".to_string(),
                remote_hash: Some("abc1234".to_string()),
                freshness: BranchFreshness::Different,
            }),
            "differs from origin/main @ abc1234"
        );
        assert_eq!(
            format_branch_upstream_status(&BranchUpstreamStatus::Tracking {
                upstream: "origin/main".to_string(),
                upstream_hash: Some("abc1234".to_string()),
                ahead: 0,
                behind: 0,
            }),
            "up-to-date with origin/main @ abc1234"
        );
        assert_eq!(
            format_branch_upstream_status(&BranchUpstreamStatus::Tracking {
                upstream: "origin/main".to_string(),
                upstream_hash: None,
                ahead: 2,
                behind: 1,
            }),
            "ahead 2, behind 1 vs origin/main"
        );
    }

    #[test]
    fn parses_ls_remote_hashes() {
        assert_eq!(
            parse_ls_remote_hash("abc1234\trefs/heads/main\n", "refs/heads/main",),
            Some("abc1234".to_string())
        );
        assert_eq!(
            parse_ls_remote_hash("abc1234\trefs/heads/other\n", "refs/heads/main"),
            None
        );
    }

    #[test]
    fn formats_display_hashes() {
        assert_eq!(
            display_hash("abcdef1234567890", BranchHash::Full),
            Some("abcdef1234567890".to_string())
        );
        assert_eq!(
            display_hash("abcdef1234567890", BranchHash::Short),
            Some("abcdef1".to_string())
        );
        assert_eq!(display_hash("abcdef1234567890", BranchHash::None), None);
    }

    #[test]
    fn parses_ahead_behind_counts() {
        assert_eq!(parse_ahead_behind("2\t1\n"), Some((2, 1)));
        assert_eq!(parse_ahead_behind("0 0"), Some((0, 0)));
        assert_eq!(parse_ahead_behind("nope"), None);
    }

    #[test]
    fn query_matcher_supports_modes() {
        assert!(
            QueryMatcher::new("main", QueryMode::Contains)
                .unwrap()
                .is_match("feature/main-api")
        );
        assert!(
            QueryMatcher::new("MAIN", QueryMode::Exact)
                .unwrap()
                .is_match("main")
        );
        assert!(
            QueryMatcher::new("mai", QueryMode::Fuzzy)
                .unwrap()
                .is_match("my-api-integration")
        );
        assert!(
            QueryMatcher::new("^bwe/.+-ci$", QueryMode::Regex)
                .unwrap()
                .is_match("bwe/main-api-ci")
        );
        assert!(
            QueryMatcher::new("main", QueryMode::Matches)
                .unwrap()
                .is_match("feature/main-api")
        );
    }

    #[test]
    fn query_matcher_reports_match_ranges() {
        assert_eq!(
            QueryMatcher::new("main", QueryMode::Contains)
                .unwrap()
                .match_ranges("feature/main-api"),
            vec![(8, 12)]
        );
        assert_eq!(
            QueryMatcher::new("mn", QueryMode::Fuzzy)
                .unwrap()
                .match_ranges("main"),
            vec![(0, 1), (3, 4)]
        );
        assert_eq!(
            QueryMatcher::new("m.*n", QueryMode::Regex)
                .unwrap()
                .match_ranges("main"),
            vec![(0, 4)]
        );
    }

    #[test]
    fn matched_text_reports_query_ranges() {
        let matcher = QueryMatcher::new("gfind", QueryMode::Contains).unwrap();
        let matched = matched_text("git@github.com:bwestlin/gfind.git", &matcher).unwrap();

        assert_eq!(matched.ranges, vec![(24, 29)]);
    }

    #[test]
    fn directory_exclusions_match_names_and_relative_paths() {
        let exclusions = DirectoryExclusions::new(vec![
            "^target$".to_string(),
            "^vendor/generated$".to_string(),
        ])
        .unwrap();
        let root = Path::new("/work");

        assert!(exclusions.matches(root, Path::new("/work/project/target")));
        assert!(exclusions.matches(root, Path::new("/work/vendor/generated")));
        assert!(!exclusions.matches(root, Path::new("/work/project/targeted")));
        assert!(!exclusions.matches(root, Path::new("/work/vendor/source")));
    }

    #[test]
    fn output_highlight_uses_dark_gray_background_for_matched_text() {
        let style = OutputStyle { enabled: true };
        let highlighted = style.highlight_plain("feature/main", &[(8, 12)]);

        assert!(highlighted.contains("\x1b[100m"));
        assert!(!highlighted.contains("\x1b[1m"));
        assert!(!highlighted.contains("\x1b[4m"));
        assert!(highlighted.contains("main"));
    }

    #[test]
    fn documented_config_is_valid_and_complete() {
        let config: ConfigFile = toml::from_str(documented_config()).unwrap();

        assert_eq!(config.paths, Some(Vec::new()));
        assert_eq!(config.include_cwd, Some(true));
        assert_eq!(config.absolute_paths, Some(false));
        assert_eq!(config.exclude_dirs, Some(vec!["^target$".to_string()]));
        assert_eq!(config.branch_search, Some(BranchSearch::Local));
        assert_eq!(config.repo_query, Some(QueryMode::Contains));
        assert_eq!(config.branch_query, Some(QueryMode::Contains));
        assert_eq!(config.stash_query, Some(QueryMode::Contains));
        assert_eq!(config.branch_hash, Some(BranchHash::Full));
        assert_eq!(config.branch_upstream_status, Some(true));
        assert_eq!(config.branch_origin_status, Some(false));

        for key in [
            "paths",
            "include_cwd",
            "absolute_paths",
            "exclude_dirs",
            "branch_search",
            "repo_query",
            "branch_query",
            "stash_query",
            "branch_hash",
            "branch_upstream_status",
            "branch_origin_status",
            "branch_hashes",
            "Examples",
            "paths = [\"~/src\", \"~/work\"]",
        ] {
            assert!(documented_config().contains(key), "missing {key}");
        }
    }

    #[test]
    fn branch_name_highlighting_uses_matched_branch_part() {
        assert_eq!(
            QueryMatcher::new("main", QueryMode::Exact)
                .unwrap()
                .branch_match_ranges("origin/main"),
            vec![(7, 11)]
        );
        assert_eq!(
            QueryMatcher::new("api", QueryMode::Contains)
                .unwrap()
                .branch_match_ranges("feature/main-api"),
            vec![(13, 16)]
        );
    }
}
