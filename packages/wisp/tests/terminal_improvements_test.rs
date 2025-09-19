use wisp::ui::SimpleSpinner;

#[test]
fn test_simple_spinner_no_concurrent_writes() {
    let mut spinner = SimpleSpinner::new("test_tool", "test_model", "testing...");

    // Test that spinner starts and stops properly
    assert!(!spinner.is_running());
    spinner.start();
    assert!(spinner.is_running());
    spinner.stop();
    assert!(!spinner.is_running());
}

#[test]
fn test_simple_spinner_frame_advancement() {
    let mut spinner = SimpleSpinner::new("test_tool", "test_model", "testing...");
    spinner.start();

    // Test that advance_frame works correctly
    spinner.advance_frame();

    // Test that frame advancement doesn't panic
    for _ in 0..20 {
        spinner.advance_frame();
    }

    // Should still be running after many frame advances
    assert!(spinner.is_running());
}

#[test]
fn test_simple_spinner_render_without_panic() {
    let mut spinner = SimpleSpinner::new("test_tool", "test_model", "testing...");
    let mut buffer = Vec::new();

    // Should not panic when rendering stopped spinner
    spinner.render(&mut buffer).unwrap();

    spinner.start();
    // Should not panic when rendering running spinner
    spinner.render(&mut buffer).unwrap();

    spinner.stop();
    // Should not panic after stopping
    spinner.render(&mut buffer).unwrap();
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_no_concurrent_terminal_writes() {
        // This test simulates multiple spinners to ensure they don't cause
        // race conditions when rendered sequentially
        let output = Arc::new(Mutex::new(Vec::new()));
        let mut spinners = vec![];

        for i in 0..5 {
            let mut spinner = SimpleSpinner::new(&format!("tool_{}", i), "model", "running");
            spinner.start();
            spinners.push(spinner);
        }

        // Simulate multiple render cycles
        for _ in 0..10 {
            let output_clone = Arc::clone(&output);
            let mut buffer = output_clone.lock().unwrap();

            for spinner in &mut spinners {
                spinner.advance_frame();
                // Each spinner renders to the same buffer sequentially
                // This should not cause any race conditions since we removed async tasks
                spinner.render(&mut *buffer).unwrap();
            }

            // Brief pause to simulate real usage
            thread::sleep(Duration::from_millis(10));
        }

        // All spinners should still be in valid state
        for spinner in &spinners {
            assert!(spinner.is_running());
        }
    }
}