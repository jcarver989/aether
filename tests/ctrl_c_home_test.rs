use aether::action::Action;
use aether::components::Component;
use aether::components::home::Home;
use tokio::sync::mpsc;

// NOTE: These tests now verify that Home component no longer handles key events directly.
// All key handling has been centralized in app.rs for better architecture.

#[test]
fn test_home_no_longer_handles_key_events() {
    let mut home = Home::new();
    let (tx, _rx) = mpsc::unbounded_channel();
    home.register_action_handler(tx)
        .expect("Failed to register action handler");

    // Home component should no longer have handle_key_event method or should return None
    // Since we removed handle_key_event entirely, we test that actions work instead

    // Test that Home properly processes actions that would have come from centralized key handling
    let result = home
        .update(Action::InsertChar('q'))
        .expect("Failed to update with InsertChar");
    assert_eq!(
        result, None,
        "Home should forward actions to children, not return new actions"
    );

    // Test that quit-related actions would be handled at the app level, not component level
    let result = home
        .update(Action::Quit)
        .expect("Failed to update with Quit");
    assert_eq!(result, None, "Home should not intercept Quit actions");
}

#[test]
fn test_home_processes_actions_correctly() {
    let mut home = Home::new();
    let (tx, _rx) = mpsc::unbounded_channel();
    home.register_action_handler(tx)
        .expect("Failed to register action handler");

    // Test that Home correctly processes various actions by forwarding to child components
    let result = home
        .update(Action::InsertChar('c'))
        .expect("Failed to update with InsertChar");
    assert_eq!(
        result, None,
        "Home should forward actions without modification"
    );

    let result = home
        .update(Action::ClearInput)
        .expect("Failed to update with ClearInput");
    assert_eq!(
        result, None,
        "Home should forward actions without modification"
    );
}
