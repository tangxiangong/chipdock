//! Filters release announcements before Sparkle and Velopack package them.

use std::{env, fs, path::Path};

use anyhow::{Context, Result, bail, ensure};

const PLATFORMS: [&str; 3] = ["macos", "windows", "linux"];

fn main() -> Result<()> {
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    let [platform, source, destination] = arguments.as_slice() else {
        bail!("usage: chipdock-release-notes <macos|windows|linux> <source.md> <output.md>");
    };
    let platform = platform.to_str().context("Platform must be valid UTF-8")?;
    write_notes(platform, Path::new(source), Path::new(destination))
}

fn write_notes(platform: &str, source: &Path, destination: &Path) -> Result<()> {
    let markdown = fs::read_to_string(source)
        .with_context(|| format!("Could not read release notes: {}", source.display()))?;
    let selected = select(&markdown, platform)
        .with_context(|| format!("Invalid release notes: {}", source.display()))?;
    ensure!(
        !selected.trim().is_empty(),
        "Release notes for {platform} must not be empty: {}",
        source.display()
    );
    if let Some(parent) = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("Could not create output directory: {}", parent.display()))?;
    }
    fs::write(destination, selected)
        .with_context(|| format!("Could not write release notes: {}", destination.display()))
}

/// Uses BibCiTeX's platform markers, validating even blocks excluded from output.
fn select(markdown: &str, platform: &str) -> Result<String> {
    ensure!(
        PLATFORMS.contains(&platform),
        "Unknown target platform: {platform}"
    );
    let mut selected = None;
    let mut output = String::new();
    let markdown = markdown.strip_prefix('\u{feff}').unwrap_or(markdown);
    for (index, line) in markdown.split_inclusive('\n').enumerate() {
        let line_number = index + 1;
        let directive = line.trim();
        if let Some(targets) = directive
            .strip_prefix("<!-- platform:")
            .and_then(|value| value.strip_suffix(" -->"))
        {
            ensure!(
                selected.is_none(),
                "Line {line_number}: Nested platform blocks are forbidden"
            );
            let targets: Vec<_> = targets.split(',').map(str::trim).collect();
            ensure!(
                targets
                    .iter()
                    .all(|target| *target == "all" || PLATFORMS.contains(target)),
                "Line {line_number}: Unknown or empty platform selector"
            );
            ensure!(
                !targets.contains(&"all") || targets.len() == 1,
                "Line {line_number}: Use all alone"
            );
            selected = Some((
                targets.contains(&"all") || targets.contains(&platform),
                line_number,
            ));
        } else if directive == "<!-- /platform -->" {
            ensure!(
                selected.take().is_some(),
                "Line {line_number}: Unmatched platform closing marker"
            );
        } else {
            ensure!(
                !line.split("<!--").skip(1).any(|comment| {
                    let comment = comment.trim_start().to_ascii_lowercase();
                    comment.starts_with("platform") || comment.starts_with("/platform")
                }),
                "Line {line_number}: Platform directives must use the documented syntax on a separate line"
            );
            if selected.is_none_or(|(matches, _)| matches) {
                output.push_str(line);
            }
        }
    }
    if let Some((_, line_number)) = selected {
        bail!("Line {line_number}: Unclosed platform block");
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unmarked_notes_preserve_markdown_for_every_platform() {
        let source =
            "# 更新\r\n\r\n- **共享功能**\r\n\n```text\n  code\n```\n<!-- comment -->\nlast line";
        for platform in PLATFORMS {
            assert_eq!(select(source, platform).unwrap(), source);
        }
    }

    #[test]
    fn common_and_matching_blocks_preserve_source_order() {
        let source = concat!(
            "# 更新\n- common\n",
            "<!-- platform:macos -->\n## macOS\n- mac\n<!-- /platform -->\n",
            "<!-- platform:windows -->\n## Windows\n- win\n<!-- /platform -->\n",
            "<!-- platform:macos,windows -->\n- desktop\n<!-- /platform -->\n",
            "<!-- platform:linux -->\n- linux\n<!-- /platform -->\n",
            "<!-- platform:all -->\n- all\n<!-- /platform -->\n",
            "- end"
        );
        for (platform, expected) in [
            (
                "macos",
                "# 更新\n- common\n## macOS\n- mac\n- desktop\n- all\n- end",
            ),
            (
                "windows",
                "# 更新\n- common\n## Windows\n- win\n- desktop\n- all\n- end",
            ),
            ("linux", "# 更新\n- common\n- linux\n- all\n- end"),
        ] {
            assert_eq!(select(source, platform).unwrap(), expected, "{platform}");
        }
    }

    #[test]
    fn markers_allow_surrounding_whitespace_and_crlf_and_utf8_bom() {
        let source = "\u{feff}# 更新\r\n  <!-- platform:macos, linux -->\r\nkeep\r\n\t<!-- /platform -->\r\n";
        assert_eq!(select(source, "macos").unwrap(), "# 更新\r\nkeep\r\n");
    }

    #[test]
    fn invalid_selectors_fail_before_content_is_selected() {
        for (selector, reason) in [
            ("android", "Unknown or empty platform selector"),
            ("Windows", "Unknown or empty platform selector"),
            ("", "Unknown or empty platform selector"),
            ("macos,", "Unknown or empty platform selector"),
            ("all,macos", "Use all alone"),
        ] {
            let source =
                format!("common\n<!-- platform:{selector} -->\ncontent\n<!-- /platform -->\n");
            assert_eq!(
                select(&source, "macos").unwrap_err().to_string(),
                format!("Line 2: {reason}"),
                "{selector}"
            );
        }
    }

    #[test]
    fn malformed_directives_fail_even_in_excluded_blocks() {
        for directive in [
            "<!--platform:windows-->",
            "<!--  platform:windows -->",
            "<!-- Platform:windows -->",
            "<!--\t/platform -->",
            "text <!-- platform:macos -->",
            "<!-- platform:macos --> text",
        ] {
            let source =
                format!("common\n<!-- platform:windows -->\n{directive}\n<!-- /platform -->\n");
            assert_eq!(
                select(&source, "macos").unwrap_err().to_string(),
                "Line 3: Platform directives must use the documented syntax on a separate line",
                "{directive}"
            );
        }
    }

    #[test]
    fn code_fences_do_not_disable_directive_validation() {
        let source = "```md\n<!-- platform:android -->\nexample\n<!-- /platform -->\n```\n";
        assert_eq!(
            select(source, "macos").unwrap_err().to_string(),
            "Line 2: Unknown or empty platform selector"
        );
    }

    #[test]
    fn unmatched_and_nested_blocks_fail_with_line_numbers() {
        for (source, expected) in [
            (
                "common\n<!-- /platform -->",
                "Line 2: Unmatched platform closing marker",
            ),
            (
                "common\n<!-- platform:windows -->\nwin",
                "Line 2: Unclosed platform block",
            ),
            (
                "<!-- platform:windows -->\n<!-- platform:macos -->\n<!-- /platform -->\n<!-- /platform -->",
                "Line 2: Nested platform blocks are forbidden",
            ),
        ] {
            assert_eq!(select(source, "macos").unwrap_err().to_string(), expected);
        }
    }

    #[test]
    fn unknown_target_platform_is_rejected() {
        assert!(select("common", "android").is_err());
    }

    #[test]
    fn file_generation_filters_notes_without_changing_source() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("公告 source.md");
        let destination = directory.path().join("preview/notes macos.md");
        let markdown = "# 更新\n<!-- platform:windows -->\nWindows only\n<!-- /platform -->\n";
        fs::write(&source, markdown).unwrap();
        write_notes("macos", &source, &destination).unwrap();
        assert_eq!(fs::read_to_string(destination).unwrap(), "# 更新\n");
        assert_eq!(fs::read_to_string(source).unwrap(), markdown);
    }

    #[test]
    fn invalid_notes_do_not_overwrite_existing_output() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.md");
        let destination = directory.path().join("notes.md");
        fs::write(&source, "common\n<!-- platform:windows -->\nunterminated").unwrap();
        fs::write(&destination, "previous notes").unwrap();
        assert!(write_notes("macos", &source, &destination).is_err());
        assert_eq!(fs::read_to_string(destination).unwrap(), "previous notes");
    }

    #[test]
    fn empty_filtered_notes_do_not_create_output() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.md");
        let destination = directory.path().join("preview/notes.md");
        for markdown in [
            " \r\n",
            "<!-- platform:windows -->\nwin\n<!-- /platform -->\n",
        ] {
            fs::write(&source, markdown).unwrap();
            assert!(write_notes("macos", &source, &destination).is_err());
            assert!(!destination.exists());
        }
    }
}
