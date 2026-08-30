use std::process::Command;

fn get_bin_path() -> String {
    let mut path = std::env::current_exe().expect("failed to get current test exe path");
    path.pop(); // remove test binary name
    if path.ends_with("deps") {
        path.pop(); // remove deps
    }
    path.push("agym");
    path.to_string_lossy().to_string()
}

#[test]
fn test_cli_version_flag() {
    let bin = get_bin_path();
    let output = Command::new(&bin)
        .arg("--version")
        .output()
        .expect("failed to execute agym");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn test_cli_completions() {
    let bin = get_bin_path();
    for shell in ["bash", "zsh", "fish", "powershell", "elvish"] {
        let output = Command::new(&bin)
            .args(["completions", shell])
            .output()
            .unwrap_or_else(|_| panic!("failed to generate completions for {}", shell));

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(!stdout.is_empty(), "completions for {} was empty", shell);
    }
}

#[test]
fn test_set_active_account_nonexistent_propagates_error_cleanly() {
    let bin = get_bin_path();
    let nonexistent_target = "definitely_nonexistent_user_999999@example.com";
    let output = Command::new(&bin)
        .arg(nonexistent_target)
        .output()
        .expect("failed to execute agym");

    // Must exit with non-zero status (error propagated from main -> Result)
    assert!(
        !output.status.success(),
        "expected non-zero exit code for nonexistent account"
    );
    assert_eq!(output.status.code(), Some(1));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}\n{}", stdout, stderr);

    // Must NOT contain any panic or unwrap backtrace
    assert!(
        !combined.contains("panicked at"),
        "process panicked on nonexistent account: {}",
        combined
    );
    assert!(
        !combined.contains("fatal runtime error"),
        "runtime error on nonexistent account: {}",
        combined
    );

    // Must contain informative error message
    assert!(
        combined.contains(
            "Account matching 'definitely_nonexistent_user_999999@example.com' not found"
        ),
        "missing expected error message in output: {}",
        combined
    );
}

#[test]
fn test_set_active_account_sql_injection_defense() {
    let bin = get_bin_path();
    let injection_target = "'; DROP TABLE accounts; --";
    let output = Command::new(&bin)
        .arg(injection_target)
        .output()
        .expect("failed to execute agym");

    assert!(!output.status.success());
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!combined.contains("panicked at"));
    assert!(combined.contains("Account matching ''; DROP TABLE accounts; --' not found"));
}

#[test]
fn test_set_active_account_empty_or_special_chars() {
    let bin = get_bin_path();
    let test_cases = ["!@#$%^&*()_+", "../../../../etc/passwd"];

    for input in test_cases {
        let output = Command::new(&bin)
            .arg(input)
            .output()
            .unwrap_or_else(|e| panic!("failed to execute agym with input '{}': {}", input, e));

        let combined = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            !combined.contains("panicked at"),
            "panicked on input '{}': {}",
            input,
            combined
        );
        assert!(!combined.contains("fatal runtime error"));
    }
}

#[test]
fn test_concurrent_nonexistent_account_queries_stress() {
    let bin = get_bin_path();
    let handles: Vec<_> = (0..4)
        .map(|i| {
            let bin_clone = bin.clone();
            std::thread::spawn(move || {
                let target = format!("stress_test_worker_{}@nowhere.org", i);
                let output = Command::new(&bin_clone)
                    .arg(&target)
                    .output()
                    .expect("failed to run agym in thread");

                assert!(!output.status.success());
                let combined = format!(
                    "{}\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
                assert!(!combined.contains("panicked at"));
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("thread panicked");
    }
}
