use crossterm::event::{self, Event, KeyEvent, KeyEventKind};
use std::time::Duration;

pub enum AppEvent {
    Key(KeyEvent),
    Tick,
}

pub fn poll() -> std::io::Result<Option<AppEvent>> {
    if event::poll(Duration::from_millis(200))? {
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                return Ok(Some(AppEvent::Tick));
            }
            return Ok(Some(AppEvent::Key(key)));
        }
    }
    Ok(Some(AppEvent::Tick))
}
