use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table, TableState},
    Frame,
};

pub struct StatefulTable {
    pub state: TableState,
}

impl StatefulTable {
    pub fn new() -> Self {
        let mut state = TableState::default();
        state.select(Some(0));
        Self { state }
    }

    pub fn select(&mut self, idx: usize) {
        self.state.select(Some(idx));
    }

    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        title: &str,
        headers: &[&str],
        rows: Vec<Vec<String>>,
    ) {
        let header_cells = headers.iter().map(|h| {
            Cell::from(*h).style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
        });
        let header = Row::new(header_cells).height(1);

        let rows: Vec<Row> = rows
            .into_iter()
            .map(|r| {
                let cells = r.into_iter().map(|c| Cell::from(c));
                Row::new(cells).height(1)
            })
            .collect();

        let widths: Vec<ratatui::layout::Constraint> = headers
            .iter()
            .map(|_| ratatui::layout::Constraint::Min(10))
            .collect();

        let table = Table::new(rows, widths)
            .header(header)
            .block(Block::default().borders(Borders::ALL).title(title))
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ ");

        frame.render_stateful_widget(table, area, &mut self.state);
    }
}
