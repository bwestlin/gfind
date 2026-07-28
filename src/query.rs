use clap::ValueEnum;
use regex::Regex;
use serde::Deserialize;

use crate::error::Result;
use crate::style::OutputStyle;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum QueryMode {
    Contains,
    Matches,
    Exact,
    Fuzzy,
    Regex,
}

impl QueryMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Contains => "contains",
            Self::Matches => "matches",
            Self::Exact => "exact",
            Self::Fuzzy => "fuzzy",
            Self::Regex => "regex",
        }
    }

    fn normalized(self) -> Self {
        match self {
            Self::Matches => Self::Contains,
            other => other,
        }
    }
}

#[derive(Debug)]
pub(crate) struct QueryMatcher {
    query: String,
    mode: QueryMode,
    regex: Option<Regex>,
}

impl QueryMatcher {
    pub(crate) fn new(query: &str, mode: QueryMode) -> Result<Self> {
        let mode = mode.normalized();
        let regex = if mode == QueryMode::Regex {
            Some(Regex::new(query).map_err(|err| format!("invalid regex query {query:?}: {err}"))?)
        } else {
            None
        };

        Ok(Self {
            query: query.to_string(),
            mode,
            regex,
        })
    }

    pub(crate) fn is_match(&self, value: &str) -> bool {
        match self.mode {
            QueryMode::Contains | QueryMode::Matches => {
                value.to_lowercase().contains(&self.query.to_lowercase())
            }
            QueryMode::Exact => value.eq_ignore_ascii_case(&self.query),
            QueryMode::Fuzzy => fuzzy_match(&self.query, value),
            QueryMode::Regex => self
                .regex
                .as_ref()
                .is_some_and(|regex| regex.is_match(value)),
        }
    }

    pub(crate) fn highlight_branch_name(&self, value: &str, style: &OutputStyle) -> String {
        let ranges = self.branch_match_ranges(value);
        style.highlight(value, &ranges)
    }

    pub(crate) fn branch_match_ranges(&self, value: &str) -> Vec<(usize, usize)> {
        let ranges = self.match_ranges(value);

        if !ranges.is_empty() {
            return ranges;
        }

        let Some((prefix, branch)) = value.split_once('/') else {
            return ranges;
        };
        let offset = prefix.len() + 1;

        self.match_ranges(branch)
            .into_iter()
            .map(|(start, end)| (start + offset, end + offset))
            .collect()
    }

    pub(crate) fn match_ranges(&self, value: &str) -> Vec<(usize, usize)> {
        match self.mode {
            QueryMode::Contains | QueryMode::Matches => {
                case_insensitive_substring_ranges(value, &self.query)
            }
            QueryMode::Exact => {
                if value.eq_ignore_ascii_case(&self.query) {
                    vec![(0, value.len())]
                } else {
                    Vec::new()
                }
            }
            QueryMode::Fuzzy => fuzzy_match_ranges(&self.query, value),
            QueryMode::Regex => self
                .regex
                .as_ref()
                .map(|regex| {
                    regex
                        .find_iter(value)
                        .map(|matched| (matched.start(), matched.end()))
                        .filter(|(start, end)| start < end)
                        .collect()
                })
                .unwrap_or_default(),
        }
    }
}

fn fuzzy_match(query: &str, value: &str) -> bool {
    !fuzzy_match_ranges(query, value).is_empty() || query.is_empty()
}

fn fuzzy_match_ranges(query: &str, value: &str) -> Vec<(usize, usize)> {
    let mut query_chars = query.chars().flat_map(char::to_lowercase);
    let Some(mut expected) = query_chars.next() else {
        return vec![(0, value.len())];
    };
    let mut ranges = Vec::new();

    for (index, value_char) in value.char_indices() {
        let mut lowered = value_char.to_lowercase();
        if lowered.any(|char| char == expected) {
            ranges.push((index, index + value_char.len_utf8()));
            match query_chars.next() {
                Some(next) => expected = next,
                None => return ranges,
            }
        }
    }

    Vec::new()
}

fn case_insensitive_substring_ranges(value: &str, query: &str) -> Vec<(usize, usize)> {
    if query.is_empty() {
        return vec![(0, value.len())];
    }

    let value_lower = value.to_lowercase();
    let query_lower = query.to_lowercase();
    let mut ranges = Vec::new();
    let mut search_start = 0;

    while let Some(relative_start) = value_lower[search_start..].find(&query_lower) {
        let start = search_start + relative_start;
        let end = start + query_lower.len();

        if value.is_char_boundary(start) && value.is_char_boundary(end) {
            ranges.push((start, end));
        }

        search_start = end;

        if search_start >= value_lower.len() {
            break;
        }
    }

    ranges
}
