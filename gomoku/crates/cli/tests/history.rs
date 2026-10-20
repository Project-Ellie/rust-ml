use cli::history::History;
use engine::Board;

#[test]
fn new_history_has_empty_root() {
    let history = History::new();
    assert_eq!(history.ply(), 0);
    assert_eq!(history.board(), Board::new());
    assert!(history.children(history.cursor()).is_empty());
}