use aether::action::Action;
use aether::app::Mode;
use aether::config::Config;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[test]
fn test_ctrl_c_quits_app() {
    let config = Config::new().expect("Failed to create config");

    // Get the keybinding for Home mode
    let home_bindings = config
        .keybindings
        .get(&Mode::Home)
        .expect("Home mode should exist in config");

    // Create Ctrl+C key event
    let ctrl_c_key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);

    // Check that Ctrl+C is bound to Quit action
    let action = home_bindings
        .get(&vec![ctrl_c_key])
        .expect("Ctrl+C should be bound to an action");

    assert_eq!(action, &Action::Quit, "Ctrl+C should quit the app");
}
