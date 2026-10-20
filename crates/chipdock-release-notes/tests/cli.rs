use std::{fs, process::Command};

#[test]
fn cli_writes_platform_notes_with_relative_unicode_paths() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("完整公告 source.md");
    let markdown = concat!(
        "# 更新\n- common\n",
        "<!-- platform:macos -->\n- mac\n<!-- /platform -->\n",
        "<!-- platform:windows -->\n- win\n<!-- /platform -->\n"
    );
    fs::write(&source, markdown).unwrap();
    for (platform, expected) in [
        ("macos", "# 更新\n- common\n- mac\n"),
        ("windows", "# 更新\n- common\n- win\n"),
    ] {
        let destination = format!("preview/公告 {platform}.md");
        let output = Command::new(env!("CARGO_BIN_EXE_chipdock-release-notes"))
            .current_dir(directory.path())
            .args([platform, "完整公告 source.md", &destination])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            fs::read_to_string(directory.path().join(destination)).unwrap(),
            expected
        );
    }
    assert_eq!(fs::read_to_string(source).unwrap(), markdown);
}

#[test]
fn cli_returns_failure_without_replacing_notes_for_an_invalid_hidden_block() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.md");
    let destination = directory.path().join("notes.md");
    fs::write(&source, "common\n<!-- platform:windows -->\nunterminated").unwrap();
    fs::write(&destination, "Previous notes").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_chipdock-release-notes"))
        .arg("macos")
        .arg(source)
        .arg(&destination)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Line 2: Unclosed platform block"));
    assert_eq!(fs::read_to_string(destination).unwrap(), "Previous notes");
}
