// SPDX-License-Identifier: AGPL-3.0-or-later

use super::super::app::App;
use crate::ui::theme;
use ratatui::{
    layout::Rect,
    style::Style,
    widgets::{Block, Borders, Paragraph},
};

pub(super) fn draw_metrics(frame: &mut ratatui::Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title("Metrics")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::BORDER_ACTIVE));
    frame.render_widget(Paragraph::new(metrics_text(app)).block(block), area);
}

fn metrics_text(app: &App) -> String {
    if metrics_empty(app) {
        return "(No metrics in this window yet. Run `kaizen metrics` in a shell, or `r` here after a repo is indexed.)\n\nMetrics need a successful snapshot + events for tool spans — see docs/telemetry-journey.md.".to_string();
    }
    let mut lines = vec!["Slow tools".to_string()];
    if let Some(metrics) = &app.metrics {
        metrics.slowest_tools.iter().take(4).for_each(|row| {
            let p95 = row
                .p95_ms
                .map(|value| format!("{value}ms"))
                .unwrap_or_else(|| "-".into());
            lines.push(format!("{} p95={} tok={}", row.tool, p95, row.total_tokens));
        });
        lines.push(String::new());
        lines.push("Hot files".into());
        metrics
            .hottest_files
            .iter()
            .take(4)
            .for_each(|row| lines.push(format!("{} {}", row.value, row.path)));
    }
    lines.join("\n")
}

fn metrics_empty(app: &App) -> bool {
    app.metrics.is_none()
        || app.metrics.as_ref().is_some_and(|metrics| {
            metrics.slowest_tools.is_empty() && metrics.hottest_files.is_empty()
        })
}
