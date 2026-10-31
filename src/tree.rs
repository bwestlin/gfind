use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt::Arguments;
use std::io;
use std::io::Write;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use crate::error::Result;
use crate::style::OutputStyle;

/// Streams flat output as before, buffering only when a tree is requested.
pub(crate) struct RepoOutput {
    tree: bool,
    entries: Vec<RepoEntry>,
}

struct RepoEntry {
    path: PathBuf,
    heading: String,
    lines: Vec<String>,
}

#[derive(Default)]
struct Node {
    children: BTreeMap<OsString, Self>,
    entry: Option<usize>,
    repos: usize,
}

impl RepoOutput {
    pub(crate) fn new(tree: bool) -> Self {
        Self {
            tree,
            entries: Vec::new(),
        }
    }

    pub(crate) fn repo(
        &mut self,
        path: PathBuf,
        ranges: &[(usize, usize)],
        suffix: &str,
        style: &OutputStyle,
    ) {
        let full_path = path.display().to_string();
        let label = if self.tree {
            path.components().next_back().map_or_else(
                || full_path.clone(),
                |component| component.as_os_str().to_string_lossy().into_owned(),
            )
        } else {
            full_path.clone()
        };
        // Keep highlights from full-path matches, including exact and regex queries.
        let offset = full_path.len() - label.len();
        let ranges: Vec<_> = ranges
            .iter()
            .filter_map(|&(start, end)| {
                let start = start.max(offset);
                (start < end).then_some((start - offset, end.saturating_sub(offset)))
            })
            .collect();
        let heading = format!("{}{suffix}", style.highlight_repo(&label, &ranges));

        if self.tree {
            self.entries.push(RepoEntry {
                path,
                heading,
                lines: Vec::new(),
            });
        } else {
            println!("{heading}");
        }
    }

    pub(crate) fn line(&mut self, line: Arguments<'_>) {
        if self.tree {
            if let Some(entry) = self.entries.last_mut() {
                entry.lines.push(line.to_string());
            }
        } else {
            println!("{line}");
        }
    }

    pub(crate) fn finish(self, style: &OutputStyle) -> Result<()> {
        if self.tree {
            self.write_tree(&mut io::stdout().lock(), style)
                .map_err(|err| format!("failed to write tree output: {err}"))?;
        }
        Ok(())
    }

    fn write_tree(&self, writer: &mut impl Write, style: &OutputStyle) -> io::Result<()> {
        let mut roots = BTreeMap::<PathBuf, Node>::new();
        for (index, entry) in self.entries.iter().enumerate() {
            let mut root = PathBuf::new();
            for component in entry.path.components() {
                match component {
                    Component::Prefix(_) | Component::RootDir => root.push(component.as_os_str()),
                    _ => break,
                }
            }
            if root.as_os_str().is_empty() {
                root.push(".");
            }
            let mut node = roots.entry(root).or_default();
            node.repos += 1;
            for component in entry.path.components() {
                if matches!(component, Component::Normal(_) | Component::ParentDir) {
                    node = node
                        .children
                        .entry(component.as_os_str().into())
                        .or_default();
                    node.repos += 1;
                }
            }
            node.entry = Some(index);
        }

        for (path, node) in roots {
            self.write_node(writer, &node, &path, ("", ""), style)?;
        }
        Ok(())
    }

    fn write_node(
        &self,
        writer: &mut impl Write,
        node: &Node,
        name: &Path,
        prefixes: (&str, &str),
        style: &OutputStyle,
    ) -> io::Result<()> {
        let (heading_prefix, child_prefix) = prefixes;
        if let Some(index) = node.entry {
            let entry = &self.entries[index];
            writeln!(writer, "{heading_prefix}{}", entry.heading)?;
            let guide = if node.children.is_empty() { "" } else { "│ " };
            for line in &entry.lines {
                writeln!(writer, "{child_prefix}{guide}{line}")?;
            }
        } else {
            let unit = if node.repos == 1 { "repo" } else { "repos" };
            writeln!(
                writer,
                "{heading_prefix}{} — {} {unit}",
                style.label(name.display().to_string()),
                node.repos
            )?;
        }

        for (index, (name, child)) in node.children.iter().enumerate() {
            let last = index + 1 == node.children.len();
            let branch = if last { "└── " } else { "├── " };
            let guide = if last { "    " } else { "│   " };
            self.write_node(
                writer,
                child,
                Path::new(name),
                (
                    &format!("{child_prefix}{branch}"),
                    &format!("{child_prefix}{guide}"),
                ),
                style,
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add_repo(output: &mut RepoOutput, path: &str) {
        output.repo(
            PathBuf::from(path),
            &[],
            "",
            &OutputStyle { enabled: false },
        );
    }

    fn render(output: &RepoOutput) -> String {
        let mut buffer = Vec::new();
        output
            .write_tree(&mut buffer, &OutputStyle { enabled: false })
            .unwrap();
        String::from_utf8(buffer).unwrap()
    }

    #[test]
    fn groups_and_sorts_matches_with_subtree_counts() {
        let mut output = RepoOutput::new(true);
        add_repo(&mut output, "work/web");
        add_repo(&mut output, "apps/cli");
        add_repo(&mut output, "work/api");

        assert_eq!(
            render(&output),
            concat!(
                ". — 3 repos\n",
                "├── apps — 1 repo\n",
                "│   └── cli\n",
                "└── work — 2 repos\n",
                "    ├── api\n",
                "    └── web\n",
            )
        );
    }

    #[test]
    fn retains_details_and_guides_for_nested_repos() {
        let mut output = RepoOutput::new(true);
        add_repo(&mut output, ".");
        output.line(format_args!("  local: main"));
        output.line(format_args!("    commit: abc1234"));
        add_repo(&mut output, "nested");
        output.line(format_args!("  stash@{{0}}: WIP"));
        output.line(format_args!("    +added line"));
        output.line(format_args!("    "));
        output.line(format_args!("    -removed line"));
        add_repo(&mut output, "z");

        assert_eq!(
            render(&output),
            concat!(
                ".\n",
                "│   local: main\n",
                "│     commit: abc1234\n",
                "├── nested\n",
                "│     stash@{0}: WIP\n",
                "│       +added line\n",
                "│       \n",
                "│       -removed line\n",
                "└── z\n",
            )
        );
    }

    #[test]
    fn preserves_parent_paths_and_status_suffixes() {
        let mut output = RepoOutput::new(true);
        output.repo(
            PathBuf::from("../sibling"),
            &[],
            " (dirty, ahead 2)",
            &OutputStyle { enabled: false },
        );
        add_repo(&mut output, "local");
        assert_eq!(
            render(&output),
            concat!(
                ". — 2 repos\n",
                "├── .. — 1 repo\n",
                "│   └── sibling (dirty, ahead 2)\n",
                "└── local\n",
            )
        );
    }

    #[cfg(unix)]
    #[test]
    fn absolute_paths_start_at_filesystem_root() {
        let mut output = RepoOutput::new(true);
        add_repo(&mut output, "/work/api");
        add_repo(&mut output, "/other/web");
        assert_eq!(
            render(&output),
            concat!(
                "/ — 2 repos\n",
                "├── other — 1 repo\n",
                "│   └── web\n",
                "└── work — 1 repo\n",
                "    └── api\n",
            )
        );
    }

    #[test]
    fn empty_matches_produce_no_tree() {
        assert_eq!(render(&RepoOutput::new(true)), "");
    }

    #[test]
    fn clips_full_path_highlights_to_unicode_repo_name() {
        let path = "work/räksmörgås";
        let mut output = RepoOutput::new(true);
        let style = OutputStyle { enabled: true };
        output.repo(PathBuf::from(path), &[(0, path.len())], "", &style);
        let label = "räksmörgås";
        assert_eq!(
            output.entries[0].heading,
            style.highlight_repo(label, &[(0, label.len())])
        );
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_directory_names_remain_distinct() {
        use std::os::unix::ffi::OsStringExt;

        let mut output = RepoOutput::new(true);
        for byte in [0xfe, 0xff] {
            let path = PathBuf::from(OsString::from_vec(vec![byte])).join("repo");
            output.repo(path, &[], "", &OutputStyle { enabled: false });
        }
        let rendered = render(&output);
        assert_eq!(rendered.matches("└── repo").count(), 2);
        assert!(rendered.starts_with(". — 2 repos\n"));
    }
}
