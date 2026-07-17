// The pure heart of the launchpad: filtering a Launchpad descriptor by a query.
// No terminal, no DOM — a renderer drives it.
//
// This is the Rust twin of the TypeScript filter in @savvifi/meridian-launchpad
// (src/filter.ts). The two MUST agree: the same descriptor + query has to rank
// the same way whether the palette is painted by React or by ratatui, otherwise
// "one descriptor, many modalities" is a claim rather than a property. The match
// is a case-insensitive subsequence over each command's title + subtitle +
// keywords, scored so a tight, early match ranks above a loose, late one.

use crate::proto::{Command, Launchpad};

/// A group header + the commands under it that survived (and are sorted by) the
/// current query. A lightweight view model — not a proto message — so the filter
/// never constructs descriptors (the synthetic "Suggestions" group has no proto
/// source). Borrows the descriptor: a renderer re-filters on every keystroke, so
/// the hot path stays allocation-light.
#[derive(Debug, Clone, PartialEq)]
pub struct FilteredGroup<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub commands: Vec<&'a Command>,
}

/// The text a command is matched against.
pub fn command_haystack(command: &Command) -> String {
    let mut parts: Vec<&str> = Vec::with_capacity(2 + command.keywords.len());
    if !command.title.is_empty() {
        parts.push(&command.title);
    }
    if !command.subtitle.is_empty() {
        parts.push(&command.subtitle);
    }
    for keyword in &command.keywords {
        if !keyword.is_empty() {
            parts.push(keyword);
        }
    }
    parts.join(" ")
}

/// Case-insensitive subsequence score. Returns a score (LOWER is better: reward
/// an early first hit and few gaps between matched characters) or `None` when the
/// query is not a subsequence. An empty query matches everything with score 0.
pub fn match_score(haystack: &str, query: &str) -> Option<usize> {
    if query.is_empty() {
        return Some(0);
    }
    // Lowercase the WHOLE string before indexing (as the TS side does) so the
    // char arithmetic below stays in the same space for both implementations.
    let hay: Vec<char> = haystack.to_lowercase().chars().collect();
    let mut from = 0usize;
    let mut first_idx: Option<usize> = None;
    let mut gaps = 0usize;
    let mut prev: Option<usize> = None;
    for ch in query.to_lowercase().chars() {
        let found = hay[from..].iter().position(|&h| h == ch)? + from;
        first_idx.get_or_insert(found);
        if let Some(p) = prev {
            gaps += found - p - 1;
        }
        prev = Some(found);
        from = found + 1;
    }
    Some(first_idx.unwrap_or(0) + gaps)
}

/// Resolve command ids against the descriptor, dropping ids that name no command
/// (`Launchpad.default_command_ids` is advisory).
fn resolve_commands_by_id<'a>(descriptor: &'a Launchpad, ids: &[String]) -> Vec<&'a Command> {
    ids.iter()
        .filter_map(|id| {
            descriptor
                .groups
                .iter()
                .flat_map(|group| group.commands.iter())
                .find(|command| &command.id == id)
        })
        .collect()
}

/// Filter + rank a Launchpad by `query`.
/// - Empty query: every group, in declared order. If `default_command_ids` are
///   set, a synthetic leading group surfaces those first (recents / pinned).
/// - Non-empty query: only matching commands, each group sorted by score, groups
///   with no matches dropped.
pub fn filter_launchpad<'a>(descriptor: &'a Launchpad, query: &str) -> Vec<FilteredGroup<'a>> {
    if query.is_empty() {
        let groups = descriptor.groups.iter().map(|group| FilteredGroup {
            id: &group.id,
            title: &group.title,
            commands: group.commands.iter().collect(),
        });
        let defaults = resolve_commands_by_id(descriptor, &descriptor.default_command_ids);
        if defaults.is_empty() {
            return groups.collect();
        }
        return std::iter::once(FilteredGroup {
            id: "__default__",
            title: "Suggestions",
            commands: defaults,
        })
        .chain(groups)
        .collect();
    }

    descriptor
        .groups
        .iter()
        .filter_map(|group| {
            let mut scored: Vec<(&Command, usize)> = group
                .commands
                .iter()
                .filter_map(|command| {
                    match_score(&command_haystack(command), query).map(|score| (command, score))
                })
                .collect();
            if scored.is_empty() {
                return None;
            }
            // Stable, like the TS Array#sort it mirrors: equal scores keep the
            // declared order.
            scored.sort_by_key(|&(_, score)| score);
            Some(FilteredGroup {
                id: &group.id,
                title: &group.title,
                commands: scored.into_iter().map(|(command, _)| command).collect(),
            })
        })
        .collect()
}

/// Flatten the filtered groups into the keyboard-navigable command order.
pub fn flatten<'a>(groups: &[FilteredGroup<'a>]) -> Vec<&'a Command> {
    groups
        .iter()
        .flat_map(|group| group.commands.iter().copied())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::CommandGroup;

    fn command(id: &str, title: &str) -> Command {
        Command {
            id: id.into(),
            title: title.into(),
            ..Default::default()
        }
    }

    fn demo() -> Launchpad {
        Launchpad {
            groups: vec![
                CommandGroup {
                    id: "create".into(),
                    title: "Create".into(),
                    commands: vec![command("new-product", "New product"), command("new-order", "New order")],
                },
                CommandGroup {
                    id: "navigate".into(),
                    title: "Navigate".into(),
                    commands: vec![command("products", "Products")],
                },
            ],
            placeholder: "Search or jump to…".into(),
            default_command_ids: vec![],
        }
    }

    #[test]
    fn empty_query_keeps_every_group_in_declared_order() {
        let descriptor = demo();
        let groups = filter_launchpad(&descriptor, "");
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].id, "create");
        assert_eq!(flatten(&groups).len(), 3);
    }

    #[test]
    fn default_command_ids_surface_as_a_synthetic_leading_group() {
        let mut descriptor = demo();
        descriptor.default_command_ids = vec!["products".into(), "nonexistent".into()];
        let groups = filter_launchpad(&descriptor, "");
        assert_eq!(groups[0].id, "__default__");
        assert_eq!(groups[0].title, "Suggestions");
        // The unknown id is dropped, not rendered as a hole.
        assert_eq!(groups[0].commands.len(), 1);
        assert_eq!(groups[0].commands[0].id, "products");
    }

    #[test]
    fn query_drops_groups_with_no_match() {
        let descriptor = demo();
        let groups = filter_launchpad(&descriptor, "prod");
        // "New product" matches in Create; "Products" matches in Navigate.
        assert_eq!(groups.len(), 2);
        let ids: Vec<&str> = flatten(&groups).iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, vec!["new-product", "products"]);
    }

    #[test]
    fn subsequence_matches_across_words_and_is_case_insensitive() {
        assert!(match_score("New product", "np").is_some());
        assert!(match_score("New product", "NP").is_some());
        assert!(match_score("New product", "zz").is_none());
    }

    #[test]
    fn tight_early_match_outranks_loose_late_one() {
        let tight = match_score("product", "pro").unwrap();
        let loose = match_score("a p r o duct", "pro").unwrap();
        assert!(tight < loose, "tight={tight} loose={loose}");
    }

    #[test]
    fn keywords_and_subtitle_are_matched() {
        let mut c = command("export", "Export");
        c.subtitle = "Download a CSV".into();
        c.keywords = vec!["csv".into(), "download".into()];
        assert!(match_score(&command_haystack(&c), "csv").is_some());
        assert!(match_score(&command_haystack(&c), "download").is_some());
    }

    #[test]
    fn scores_sort_commands_within_a_group() {
        let descriptor = Launchpad {
            groups: vec![CommandGroup {
                id: "g".into(),
                title: "G".into(),
                // "Order" scores worse for "or" than "Orders" does not — both start
                // at 0; use a decoy that only matches late to prove the ordering.
                commands: vec![command("late", "Zebra or"), command("early", "Orders")],
            }],
            ..Default::default()
        };
        let groups = filter_launchpad(&descriptor, "or");
        let ids: Vec<&str> = groups[0].commands.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, vec!["early", "late"]);
    }
}
