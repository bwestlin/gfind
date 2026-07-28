use std::env;
use std::io::IsTerminal;

use owo_colors::OwoColorize;

pub(crate) struct OutputStyle {
    pub(crate) enabled: bool,
}

#[derive(Clone, Copy, Debug)]
enum HighlightStyle {
    Repo,
    Branch,
    Commit,
    StashRef,
    Plain,
}

impl OutputStyle {
    pub(crate) fn auto() -> Self {
        Self {
            enabled: std::io::stdout().is_terminal() && env::var_os("NO_COLOR").is_none(),
        }
    }

    pub(crate) fn repo(&self, text: impl AsRef<str>) -> String {
        let text = text.as_ref();

        if self.enabled {
            text.cyan().bold().to_string()
        } else {
            text.to_string()
        }
    }

    pub(crate) fn label(&self, text: impl AsRef<str>) -> String {
        let text = text.as_ref();

        if self.enabled {
            text.dimmed().to_string()
        } else {
            text.to_string()
        }
    }

    pub(crate) fn commit(&self, text: impl AsRef<str>) -> String {
        let text = text.as_ref();

        if self.enabled {
            text.yellow().to_string()
        } else {
            text.to_string()
        }
    }

    pub(crate) fn branch(&self, text: impl AsRef<str>) -> String {
        let text = text.as_ref();

        if self.enabled {
            text.cyan().to_string()
        } else {
            text.to_string()
        }
    }

    pub(crate) fn stash_ref(&self, text: impl AsRef<str>) -> String {
        let text = text.as_ref();

        if self.enabled {
            text.magenta().to_string()
        } else {
            text.to_string()
        }
    }

    pub(crate) fn current(&self, text: impl AsRef<str>) -> String {
        let text = text.as_ref();

        if self.enabled {
            text.green().bold().to_string()
        } else {
            text.to_string()
        }
    }

    pub(crate) fn status(&self, text: impl AsRef<str>) -> String {
        let text = text.as_ref();
        if !self.enabled {
            return text.to_string();
        }

        if text.contains("up-to-date") {
            text.green().to_string()
        } else if text.contains("no upstream")
            || text.contains("differs")
            || text.contains("behind")
        {
            text.red().to_string()
        } else {
            text.yellow().to_string()
        }
    }

    pub(crate) fn reason(&self, text: impl AsRef<str>) -> String {
        let text = text.as_ref();

        if !self.enabled {
            return text.to_string();
        }

        if text.contains("dirty") {
            text.red().to_string()
        } else {
            text.yellow().to_string()
        }
    }

    pub(crate) fn highlight(&self, value: &str, ranges: &[(usize, usize)]) -> String {
        self.highlight_as(value, ranges, HighlightStyle::Branch)
    }

    pub(crate) fn highlight_repo(&self, value: &str, ranges: &[(usize, usize)]) -> String {
        self.highlight_as(value, ranges, HighlightStyle::Repo)
    }

    pub(crate) fn highlight_stash_ref(&self, value: &str, ranges: &[(usize, usize)]) -> String {
        self.highlight_as(value, ranges, HighlightStyle::StashRef)
    }

    pub(crate) fn highlight_commit(&self, value: &str, ranges: &[(usize, usize)]) -> String {
        self.highlight_as(value, ranges, HighlightStyle::Commit)
    }

    pub(crate) fn highlight_plain(&self, value: &str, ranges: &[(usize, usize)]) -> String {
        self.highlight_as(value, ranges, HighlightStyle::Plain)
    }

    fn highlight_as(
        &self,
        value: &str,
        ranges: &[(usize, usize)],
        style: HighlightStyle,
    ) -> String {
        if !self.enabled || ranges.is_empty() {
            return self.apply_style(value, style);
        }

        let mut result = String::new();
        let mut cursor = 0;

        for (start, end) in merged_ranges(ranges, value.len()) {
            if cursor < start {
                result.push_str(&self.apply_style(&value[cursor..start], style));
            }
            result.push_str(&self.apply_match_style(&value[start..end], style));
            cursor = end;
        }

        if cursor < value.len() {
            result.push_str(&self.apply_style(&value[cursor..], style));
        }

        result
    }

    fn apply_style(&self, value: &str, style: HighlightStyle) -> String {
        match style {
            HighlightStyle::Repo => self.repo(value),
            HighlightStyle::Branch => self.branch(value),
            HighlightStyle::Commit => self.commit(value),
            HighlightStyle::StashRef => self.stash_ref(value),
            HighlightStyle::Plain => value.to_string(),
        }
    }

    fn apply_match_style(&self, value: &str, style: HighlightStyle) -> String {
        if !self.enabled {
            return value.to_string();
        }

        match style {
            HighlightStyle::Repo | HighlightStyle::Branch => {
                value.cyan().on_bright_black().to_string()
            }
            HighlightStyle::Commit => value.yellow().on_bright_black().to_string(),
            HighlightStyle::StashRef => value.magenta().on_bright_black().to_string(),
            HighlightStyle::Plain => value.on_bright_black().to_string(),
        }
    }
}

fn merged_ranges(ranges: &[(usize, usize)], len: usize) -> Vec<(usize, usize)> {
    let mut ranges: Vec<_> = ranges
        .iter()
        .copied()
        .filter(|(start, end)| start < end && *end <= len)
        .collect();
    ranges.sort_unstable();

    let mut merged: Vec<(usize, usize)> = Vec::new();

    for (start, end) in ranges {
        if let Some((_, last_end)) = merged.last_mut()
            && start <= *last_end
        {
            *last_end = (*last_end).max(end);
            continue;
        }

        merged.push((start, end));
    }

    merged
}
