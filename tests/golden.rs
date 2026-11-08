use std::path::{Path, PathBuf};

fn golden_cases() -> Vec<PathBuf> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden");
    let mut cases = Vec::new();
    for entry in std::fs::read_dir(&directory).expect("the golden corpus must be readable") {
        let path = entry.expect("the golden entry must be readable").path();
        if path.extension().and_then(|extension| extension.to_str()) == Some("xcx") {
            cases.push(path);
        }
    }
    cases.sort();
    assert!(
        cases.len() >= 4,
        "the golden corpus must cover the documented example programs"
    );
    cases
}

fn run_program(source_path: &Path, profile: &str) -> std::process::Output {
    let child = std::process::Command::new(env!("CARGO_BIN_EXE_xcx-legacy"))
        .arg("run")
        .arg(profile)
        .arg(source_path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("the xcx binary must be spawnable");
    child.wait_with_output().expect("the xcx binary must finish")
}

fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn matches(golden_line: &str, actual_line: &str) -> bool {
    match golden_line.strip_suffix('*') {
        Some(prefix) => actual_line.starts_with(prefix),
        None => golden_line == actual_line,
    }
}

#[test]
fn every_documented_example_program_matches_its_golden_record() {
    for (case, profile) in golden_cases().into_iter().flat_map(|case| {
        ["--debug", "--release"].map(|profile| (case.clone(), profile))
    }) {
        let name = case
            .file_stem()
            .and_then(|stem| stem.to_str())
            .expect("the golden case has a name")
            .to_string();

        let output = run_program(&case, profile);
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

        let expected_code: i32 = match std::fs::read_to_string(case.with_extension("code")) {
            Ok(content) => content.trim().parse().expect("the golden code must parse"),
            Err(_) => 0,
        };
        assert_eq!(
            output.status.code(),
            Some(expected_code),
            "exit code mismatch for `{name}`\nstdout:\n{stdout}\nstderr:\n{stderr}"
        );

        let golden_out = normalize(
            &std::fs::read_to_string(case.with_extension("out"))
                .expect("the golden stdout must be readable"),
        );
        let stdout = normalize(&stdout);
        let actual_lines: Vec<&str> = stdout.lines().collect();
        let golden_lines: Vec<&str> = golden_out.lines().collect();
        assert_eq!(
            actual_lines.len(),
            golden_lines.len(),
            "line count mismatch for `{name}`\nexpected:\n{golden_out}\nactual:\n{stdout}"
        );
        for (golden_line, actual_line) in golden_lines.iter().zip(&actual_lines) {
            assert!(
                matches(golden_line, actual_line),
                "output mismatch for `{name}`:\nexpected line: {golden_line:?}\nactual line:   {actual_line:?}"
            );
        }

        let golden_err_path = case.with_extension("err");
        if golden_err_path.exists() {
            let golden_err = normalize(
                &std::fs::read_to_string(&golden_err_path)
                    .expect("the golden stderr must be readable"),
            );
            assert_eq!(normalize(&stderr), golden_err, "stderr mismatch for `{name}`");
        }
    }
}
