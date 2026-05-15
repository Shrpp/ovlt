use ratatui::{layout::Rect, Frame};

use crate::{app::App, components::table::StatefulTable};

pub fn render(frame: &mut Frame, app: &App, area: Rect, table: &mut StatefulTable) {
    let tenant_name = app.active_tenant_name().unwrap_or("?");
    let title = format!(" Audit Log — {tenant_name}  [x] export CSV ");

    let rows: Vec<Vec<String>> = app
        .audit_log
        .iter()
        .map(|e| {
            let ts = e
                .created_at
                .get(..19)
                .unwrap_or(&e.created_at)
                .replace('T', " ");
            vec![
                ts,
                e.action.clone(),
                e.user_id
                    .as_deref()
                    .map(|u| u[..8].to_string() + "…")
                    .unwrap_or_else(|| "—".into()),
                e.ip.clone().unwrap_or_else(|| "—".into()),
                e.metadata.clone().unwrap_or_else(|| "—".into()),
            ]
        })
        .collect();

    table.select(app.audit_log_selected);
    table.render(
        frame,
        area,
        &title,
        &["Time", "Action", "User", "IP", "Metadata"],
        rows,
    );
}
