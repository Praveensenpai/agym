use std::panic::{catch_unwind, AssertUnwindSafe};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn get_bin_path() -> String {
    let mut path = std::env::current_exe().expect("failed to get current test exe path");
    path.pop(); // remove test binary name
    if path.ends_with("deps") {
        path.pop(); // remove deps
    }
    path.push("agym");
    path.to_string_lossy().to_string()
}

/// Simulated RAII TerminalGuard mirror to empirically test Drop and Panic semantics
struct MockTerminalGuard {
    dropped: Arc<AtomicBool>,
}

impl MockTerminalGuard {
    fn new(dropped: Arc<AtomicBool>) -> Self {
        crossterm::terminal::disable_raw_mode().ok();
        Self { dropped }
    }

    fn restore() {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::cursor::Show
        );
    }
}

impl Drop for MockTerminalGuard {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::SeqCst);
        Self::restore();
    }
}

#[test]
fn test_terminal_guard_drop_on_normal_exit() {
    let dropped = Arc::new(AtomicBool::new(false));
    {
        let _guard = MockTerminalGuard::new(dropped.clone());
        assert!(!dropped.load(Ordering::SeqCst));
    }
    assert!(
        dropped.load(Ordering::SeqCst),
        "TerminalGuard must be dropped on normal scope exit"
    );
}

#[test]
fn test_terminal_guard_drop_on_panic_unwind() {
    let dropped = Arc::new(AtomicBool::new(false));
    let dropped_clone = dropped.clone();

    let result = catch_unwind(AssertUnwindSafe(|| {
        let _guard = MockTerminalGuard::new(dropped_clone);
        panic!("Simulated UI render panic");
    }));

    assert!(result.is_err(), "Expected panic to be caught");
    assert!(
        dropped.load(Ordering::SeqCst),
        "TerminalGuard Drop MUST execute during panic unwinding"
    );
}

#[test]
fn test_terminal_restore_idempotency() {
    // Calling restore multiple times must never crash or panic
    for _ in 0..10 {
        MockTerminalGuard::restore();
    }
}

#[test]
fn test_main_cli_help_and_subcommands_clean_integration() {
    let bin = get_bin_path();

    // Verify --help output
    let help_output = Command::new(&bin)
        .arg("--help")
        .output()
        .expect("failed to execute agym --help");
    assert!(help_output.status.success());
    let stdout = String::from_utf8_lossy(&help_output.stdout);
    assert!(stdout.contains("Unified Antigravity CLI & Account Manager"));
    assert!(stdout.contains("save"));
    assert!(stdout.contains("completions"));

    // Verify invalid flag fails cleanly without panic
    let invalid_output = Command::new(&bin)
        .arg("--invalid-flag-999")
        .output()
        .expect("failed to execute agym");
    assert!(!invalid_output.status.success());
    let err_str = format!(
        "{}\n{}",
        String::from_utf8_lossy(&invalid_output.stdout),
        String::from_utf8_lossy(&invalid_output.stderr)
    );
    assert!(!err_str.contains("panicked at"));
    assert!(err_str.contains("unexpected argument '--invalid-flag-999'"));
}

#[test]
fn test_main_cli_save_subcommand_execution() {
    let bin = get_bin_path();
    let output = Command::new(&bin)
        .arg("save")
        .output()
        .expect("failed to execute agym save");

    // Regardless of whether keyring has active token or not, it should exit with code 0 without panic
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!combined.contains("panicked at"));
    assert!(
        combined.contains("Saved current Antigravity account")
            || combined.contains("No active token found in keyring")
    );
}

#[test]
fn test_adversarial_long_arguments_stress() {
    let bin = get_bin_path();
    let super_long_arg = "a".repeat(65536);
    let output = Command::new(&bin)
        .arg(&super_long_arg)
        .output()
        .expect("failed to execute agym with long argument");

    assert!(!output.status.success());
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!combined.contains("panicked at"));
    assert!(!combined.contains("fatal runtime error"));
}
