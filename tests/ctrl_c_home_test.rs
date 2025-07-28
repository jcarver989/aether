use aether::components::home::Home;
use aether::components::Component;
use aether::action::Action;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[test]
fn test_home_ctrl_c_quits() {
    let mut home = Home::new();
    
    // Create Ctrl+C key event
    let ctrl_c_key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    
    // Test that Ctrl+C produces Quit action
    let result = home.handle_key_event(ctrl_c_key).expect("Failed to handle Ctrl+C");
    
    assert_eq!(result, Some(Action::Quit), "Ctrl+C should produce Quit action");
}

#[test]
fn test_home_ctrl_d_quits() {
    let mut home = Home::new();
    
    // Create Ctrl+D key event
    let ctrl_d_key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL);
    
    // Test that Ctrl+D produces Quit action
    let result = home.handle_key_event(ctrl_d_key).expect("Failed to handle Ctrl+D");
    
    assert_eq!(result, Some(Action::Quit), "Ctrl+D should produce Quit action");
}

#[test]
fn test_home_q_inserts_character_not_quit() {
    let mut home = Home::new();
    
    // Create 'q' key event
    let q_key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
    
    // Test that 'q' produces InsertChar action (should be handled by input component)
    let result = home.handle_key_event(q_key).expect("Failed to handle 'q'");
    
    assert_eq!(result, Some(Action::InsertChar('q')), "'q' should insert character, not quit");
}