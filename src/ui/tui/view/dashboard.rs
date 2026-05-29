// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::ui::theme;
use crate::visualization::VisualizationReport;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Sparkline},
};

pub fn render_dashboard(f: &mut ratatui::Frame, report: Option<&VisualizationReport>, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(3)])
        .split(area);
    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(25); 4])
        .split(rows[0]);
    kpis(report)
        .into_iter()
        .zip(top.iter())
        .for_each(|(text, rect)| render_card(f, &text, *rect));
    render_activity(f, report, rows[1]);
}

fn render_card(f: &mut ratatui::Frame, text: &str, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::BORDER_INACTIVE));
    f.render_widget(Paragraph::new(text.to_string()).block(block), area);
}

fn render_activity(f: &mut ratatui::Frame, report: Option<&VisualizationReport>, area: Rect) {
    let data: Vec<u64> = report
        .map(|r| r.activity.day_bins.iter().map(|b| b.event_count).collect())
        .unwrap_or_default();
    let block = Block::default()
        .title(activity_title(report))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::BORDER_INACTIVE));
    f.render_widget(
        Sparkline::default()
            .block(block)
            .data(&data)
            .style(Style::default().fg(Color::Cyan)),
        area,
    );
}

fn activity_title(report: Option<&VisualizationReport>) -> Line<'static> {
    let quality = report
        .map(|r| format!(" quality {:.0}% cost", r.quality.cost_coverage_pct))
        .unwrap_or_else(|| " loading".to_string());
    Line::from(vec![
        Span::raw("Activity"),
        Span::styled(quality, Style::default().fg(Color::Gray)),
    ])
}

fn kpis(report: Option<&VisualizationReport>) -> [String; 4] {
    report.map(kpi_values).unwrap_or([
        "Sessions\n-".into(),
        "Cost\n-".into(),
        "Tokens\n-".into(),
        "Errors\n-".into(),
    ])
}

fn kpi_values(report: &VisualizationReport) -> [String; 4] {
    [
        format!("Sessions\n{}", report.totals.session_count),
        format!(
            "Cost\n${:.4}",
            report.totals.cost_usd_e6 as f64 / 1_000_000.0
        ),
        format!("Tokens\n{}", compact(report.totals.tokens.total)),
        format!("Errors\n{}", report.totals.error_count),
    ]
}

fn compact(n: u64) -> String {
    match n {
        n if n >= 1_000_000 => format!("{:.1}m", n as f64 / 1_000_000.0),
        n if n >= 1_000 => format!("{:.1}k", n as f64 / 1_000.0),
        n => n.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::visualization::{TokenTotals, VisualizationTotals};

    #[test]
    fn kpis_handle_empty_report() {
        assert_eq!(kpis(None)[0], "Sessions\n-");
    }

    #[test]
    fn kpis_format_report_totals() {
        let report = report(42, 12_345);
        assert_eq!(kpis(Some(&report))[0], "Sessions\n42");
        assert_eq!(kpis(Some(&report))[2], "Tokens\n12.3k");
    }

    #[test]
    fn render_smoke_handles_empty_report() {
        let backend = ratatui::backend::TestBackend::new(80, 10);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render_dashboard(f, None, f.area()))
            .unwrap();
    }

    fn report(sessions: u64, tokens: u64) -> VisualizationReport {
        VisualizationReport {
            totals: VisualizationTotals {
                session_count: sessions,
                tokens: TokenTotals {
                    total: tokens,
                    ..TokenTotals::default()
                },
                ..VisualizationTotals::default()
            },
            ..VisualizationReport::default()
        }
    }
}
