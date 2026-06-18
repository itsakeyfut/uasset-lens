use std::path::Path;

use anyhow::Context;
use globset::GlobMatcher;

/// One parsed rule from `.uasset-lens-ignore`.
struct IgnoreRule {
    negate: bool,
    matcher: Matcher,
}

enum Matcher {
    /// Prefix match against the project-relative path (pattern has no glob metacharacters).
    Prefix(String),
    /// Glob match (`*`, `?`, `**`). `*`/`?` do not cross `/`; `**` does.
    Glob(GlobMatcher),
}

/// Ordered exclusion rules loaded from `.uasset-lens-ignore`. Patterns are matched against
/// the project-directory-relative asset path (forward-slash normalized), case-insensitively
/// on Windows. The last matching rule wins; a matching negation (`!`) re-includes the path.
#[derive(Default)]
pub(crate) struct IgnoreRules {
    rules: Vec<IgnoreRule>,
}

impl IgnoreRules {
    /// Loads `<project_dir>/.uasset-lens-ignore`. Returns empty rules when the file is absent.
    pub(crate) fn load(project_dir: &Path) -> anyhow::Result<Self> {
        let path = project_dir.join(".uasset-lens-ignore");
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e).with_context(|| format!("failed to read {}", path.display())),
        };

        let mut rules = Vec::new();
        for (i, raw) in content.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (negate, pattern) = match line.strip_prefix('!') {
                Some(rest) => (true, rest.trim()),
                None => (false, line),
            };
            if pattern.is_empty() {
                continue;
            }
            let matcher = if pattern.contains('*') || pattern.contains('?') {
                let glob = globset::GlobBuilder::new(pattern)
                    .literal_separator(true)
                    .case_insensitive(cfg!(windows))
                    .build()
                    .with_context(|| {
                        format!(
                            "invalid glob in .uasset-lens-ignore (line {}): '{}'",
                            i + 1,
                            pattern
                        )
                    })?
                    .compile_matcher();
                Matcher::Glob(glob)
            } else {
                Matcher::Prefix(normalize_case(pattern))
            };
            rules.push(IgnoreRule { negate, matcher });
        }
        Ok(Self { rules })
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Returns true if `rel_path` (project-relative, forward-slash normalized) is excluded.
    pub(crate) fn is_ignored(&self, rel_path: &str) -> bool {
        let normalized = normalize_case(rel_path);
        let mut ignored = false;
        for rule in &self.rules {
            let matched = match &rule.matcher {
                Matcher::Prefix(prefix) => normalized.starts_with(prefix.as_str()),
                Matcher::Glob(glob) => glob.is_match(rel_path),
            };
            if matched {
                ignored = !rule.negate;
            }
        }
        ignored
    }
}

/// Lowercases on Windows for case-insensitive matching; identity elsewhere.
fn normalize_case(s: &str) -> String {
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules_from(tag: &str, lines: &[&str]) -> IgnoreRules {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_ignore_{}_{}", tag, std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(".uasset-lens-ignore"), lines.join("\n")).unwrap();
        let rules = IgnoreRules::load(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        rules
    }

    #[test]
    fn load_should_return_empty_when_file_absent() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_ignore_absent_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let rules = IgnoreRules::load(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(rules.is_empty());
        assert!(!rules.is_ignored("Content/Foo.uasset"));
    }

    #[test]
    fn is_ignored_should_match_directory_prefix() {
        let rules = rules_from("prefix", &["Content/Dev/"]);
        assert!(rules.is_ignored("Content/Dev/BP_X.uasset"));
        assert!(!rules.is_ignored("Content/Maps/BP_Y.uasset"));
    }

    #[test]
    fn is_ignored_should_match_glob_with_double_star() {
        let rules = rules_from("dstar", &["Content/**/Test*.uasset"]);
        assert!(rules.is_ignored("Content/A/B/TestRig.uasset"));
        assert!(rules.is_ignored("Content/TestRig.uasset"));
        assert!(!rules.is_ignored("Content/A/BP_Real.uasset"));
    }

    #[test]
    fn is_ignored_should_re_include_with_negation_as_last_match() {
        let rules = rules_from("negate", &["Content/Dev/", "!Content/Dev/BP_Keep.uasset"]);
        assert!(rules.is_ignored("Content/Dev/BP_Other.uasset"));
        assert!(
            !rules.is_ignored("Content/Dev/BP_Keep.uasset"),
            "negation should re-include the kept asset"
        );
    }

    #[test]
    fn is_ignored_should_apply_last_matching_rule() {
        // A later exclusion overrides an earlier negation.
        let rules = rules_from(
            "lastwin",
            &[
                "Content/Dev/",
                "!Content/Dev/Keep/",
                "Content/Dev/Keep/Tmp/",
            ],
        );
        assert!(!rules.is_ignored("Content/Dev/Keep/BP_A.uasset"));
        assert!(rules.is_ignored("Content/Dev/Keep/Tmp/BP_B.uasset"));
    }

    #[test]
    fn load_should_skip_comments_and_blank_lines() {
        let rules = rules_from("comments", &["# a comment", "", "   ", "Content/QA/"]);
        assert!(rules.is_ignored("Content/QA/BP_Z.uasset"));
        assert!(!rules.is_ignored("Content/Other/BP_Z.uasset"));
    }

    #[test]
    fn single_star_should_not_cross_path_separator() {
        let rules = rules_from("singlestar", &["Content/*.uasset"]);
        assert!(rules.is_ignored("Content/Foo.uasset"));
        assert!(
            !rules.is_ignored("Content/Sub/Foo.uasset"),
            "a single * must not cross /"
        );
    }

    #[test]
    fn load_should_error_on_invalid_glob() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_ignore_bad_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // Contains `*` (routes to the glob branch) with an unterminated character class.
        std::fs::write(dir.join(".uasset-lens-ignore"), "Content/*[bad\n").unwrap();
        let result = IgnoreRules::load(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            result.is_err(),
            "an unterminated character class should fail to parse"
        );
    }
}
