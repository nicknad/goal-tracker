//! Rendering for the terminal UI.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, Cell, Paragraph, Row, Table, TableState, Tabs, Wrap};
use unicode_truncate::UnicodeTruncateStr;
use unicode_width::UnicodeWidthStr;

use crate::app::{App, ColumnWidth, Form, Mode, Section};

const HIGHLIGHT_SYMBOL: &str = "> ";
const COLUMN_SPACING: u16 = 1;

pub(crate) fn render(frame: &mut Frame, app: &App) {
    let footer = if matches!(app.mode, Mode::Form(_)) {
        2
    } else {
        1
    };
    let [header, body, footer_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(3),
        Constraint::Length(footer),
    ])
    .areas(frame.area());

    render_header(frame, app, header);
    match &app.mode {
        Mode::Browse => {
            render_body(frame, app, body);
            render_footer(frame, app, footer_area);
        }
        Mode::Form(form) => {
            render_form(frame, form, body);
            render_form_footer(frame, form, footer_area);
        }
    }
}

fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    let titles = Section::ALL
        .iter()
        .map(|section| Line::from(section.title()));
    let tabs = Tabs::new(titles)
        .select(app.section.index())
        .block(Block::bordered().title(" goal-tracker "))
        .highlight_style(Style::new().bold().cyan());
    frame.render_widget(tabs, area);
}

fn render_body(frame: &mut Frame, app: &App, area: Rect) {
    if let Some(detail) = app.selected_detail() {
        let [list_area, detail_area] =
            Layout::vertical([Constraint::Min(5), Constraint::Percentage(45)]).areas(area);
        render_table(frame, app, list_area);
        render_detail(frame, detail, detail_area);
    } else {
        render_table(frame, app, area);
    }
}

fn render_table(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::bordered().title(app.section.title());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.table.rows.is_empty() {
        frame.render_widget(
            Paragraph::new("No records. Press a to add one.").style(Style::new().dim()),
            inner,
        );
        return;
    }

    let widths = column_widths(inner.width, app.table.widths);
    let header = Row::new(app.table.headers.iter().enumerate().map(|(index, header)| {
        Cell::from(truncate(
            header,
            usize::from(widths.get(index).copied().unwrap_or(0)),
        ))
    }))
    .style(Style::new().bold());
    let rows = app.table.rows.iter().map(|row| {
        Row::new(row.iter().enumerate().map(|(index, cell)| {
            Cell::from(truncate(
                cell,
                usize::from(widths.get(index).copied().unwrap_or(0)),
            ))
        }))
    });
    let constraints: Vec<Constraint> = widths
        .iter()
        .map(|width| Constraint::Length(*width))
        .collect();
    let table = Table::new(rows, constraints)
        .header(header)
        .column_spacing(COLUMN_SPACING)
        .highlight_symbol(HIGHLIGHT_SYMBOL);

    let mut state = TableState::default().with_selected(Some(app.selected));
    frame.render_stateful_widget(table, inner, &mut state);
}

fn render_detail(frame: &mut Frame, detail: &[(&'static str, String)], area: Rect) {
    let mut text = String::new();
    for (label, value) in detail {
        text.push_str(label);
        text.push_str(": ");
        text.push_str(value);
        text.push('\n');
    }
    frame.render_widget(
        Paragraph::new(text)
            .block(Block::bordered().title(" Details "))
            .wrap(Wrap { trim: false }),
        area,
    );
}

/// Browse-mode key help.
const BROWSE_HELP: &str = "q quit  tab section  up/down  a add  e edit  ←/→ details  r reload";

fn render_footer(frame: &mut Frame, app: &App, area: Rect) {
    let help_width = u16::try_from(BROWSE_HELP.width()).unwrap_or(80);
    let [left, right] =
        Layout::horizontal([Constraint::Min(1), Constraint::Length(help_width)]).areas(area);

    frame.render_widget(Paragraph::new(app.status.as_str()), left);
    frame.render_widget(Paragraph::new(BROWSE_HELP).right_aligned(), right);
}

fn render_form(frame: &mut Frame, form: &Form, area: Rect) {
    let total = usize::from(area.width);
    let label = 22.min(total);
    let value_width = total
        .saturating_sub(HIGHLIGHT_SYMBOL.width())
        .saturating_sub(usize::from(COLUMN_SPACING))
        .saturating_sub(label);

    let rows = form.fields.iter().enumerate().map(|(index, field)| {
        let label = if field.optional {
            format!("{} (optional)", field.label)
        } else {
            field.label.to_owned()
        };
        let value = if index == form.selected {
            truncate_start(&field.value, value_width)
        } else {
            truncate(&field.value, value_width)
        };
        Row::new(vec![Cell::from(label), Cell::from(value)])
    });
    let widths = [
        Constraint::Length(u16::try_from(label).unwrap_or(0)),
        Constraint::Length(u16::try_from(value_width).unwrap_or(0)),
    ];
    let table = Table::new(rows, widths)
        .block(Block::bordered().title(format!(" {} ", form.title)))
        .column_spacing(COLUMN_SPACING)
        .highlight_symbol(HIGHLIGHT_SYMBOL)
        .row_highlight_style(Style::new().reversed());

    let mut state = TableState::default().with_selected(Some(form.selected));
    frame.render_stateful_widget(table, area, &mut state);
}

fn render_form_footer(frame: &mut Frame, form: &Form, area: Rect) {
    let [help_area, message_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(area);

    frame.render_widget(
        Paragraph::new("enter save  esc cancel  tab next  backspace delete"),
        help_area,
    );

    let message = form.error.clone().unwrap_or_else(|| {
        let choices = form.choices();
        if choices.is_empty() {
            String::new()
        } else {
            format!("choices: {}", choices.join(" | "))
        }
    });
    if !message.is_empty() {
        let style = if form.error.is_some() {
            Style::new().red()
        } else {
            Style::new().dim()
        };
        frame.render_widget(Paragraph::new(message).style(style), message_area);
    }
}

/// Width of each table column, excluding the highlight symbol and spacing.
///
/// Fixed columns take their width first; fill columns share what remains.
fn column_widths(total: u16, spec: &[ColumnWidth]) -> Vec<u16> {
    let count = spec.len().max(1);
    let spacing = usize::from(COLUMN_SPACING) * count.saturating_sub(1);
    let available = usize::from(total)
        .saturating_sub(HIGHLIGHT_SYMBOL.width())
        .saturating_sub(spacing);

    let mut fixed = 0_usize;
    let mut widths = spec
        .iter()
        .map(|width| match width {
            ColumnWidth::Length(length) => {
                fixed += usize::from(*length);
                usize::from(*length)
            }
            ColumnWidth::Fill | ColumnWidth::FillMin(_) => 0,
        })
        .collect::<Vec<_>>();

    let fill_columns = std::num::NonZeroUsize::new(
        spec.iter()
            .filter(|width| matches!(width, ColumnWidth::Fill | ColumnWidth::FillMin(_)))
            .count(),
    );
    if let Some(fill_columns) = fill_columns {
        let remaining = available.saturating_sub(fixed);
        let share = remaining / fill_columns;
        let extra = remaining % fill_columns;
        let mut filled = 0;
        for (slot, width) in widths.iter_mut().enumerate() {
            let policy = spec[slot];
            if matches!(policy, ColumnWidth::Fill | ColumnWidth::FillMin(_)) {
                let minimum = match policy {
                    ColumnWidth::FillMin(percent) => available * usize::from(percent) / 100,
                    ColumnWidth::Length(_) | ColumnWidth::Fill => 0,
                };
                *width = (share + usize::from(filled < extra)).max(minimum);
                filled += 1;
            }
        }
    }

    widths
        .into_iter()
        .map(|width| u16::try_from(width).unwrap_or(0))
        .collect()
}

/// Shortens `value` to `width` columns, keeping the start.
fn truncate(value: &str, width: usize) -> String {
    if value.width() <= width {
        return value.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    let (head, _) = value.unicode_truncate(width - 1);
    format!("{head}…")
}

/// Shortens `value` to `width` columns, keeping the end.
fn truncate_start(value: &str, width: usize) -> String {
    if value.width() <= width {
        return value.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    let (tail, _) = value.unicode_truncate_start(width - 1);
    format!("…{tail}")
}

#[cfg(test)]
mod tests {
    use super::{column_widths, truncate, truncate_start};
    use crate::app::ColumnWidth;

    #[test]
    fn truncation_keeps_the_ellipsis_within_the_width() {
        assert_eq!(truncate("hello", 5), "hello");
        assert_eq!(truncate("hello", 4), "hel…");
        assert_eq!(truncate("hello", 0), "");
        assert_eq!(truncate("你好吗", 4), "你…");
        assert_eq!(truncate_start("hello", 5), "hello");
        assert_eq!(truncate_start("hello", 4), "…llo");
        assert_eq!(truncate_start("hello", 0), "");
        assert_eq!(truncate_start("你好吗", 4), "…吗");
    }

    #[test]
    fn goal_title_takes_the_space_after_fixed_columns() {
        let spec = [
            ColumnWidth::FillMin(60),
            ColumnWidth::Length(10),
            ColumnWidth::Length(10),
        ];
        // 120 total - 2 highlight - 2 spacing = 116 available.
        assert_eq!(column_widths(120, &spec), vec![96, 10, 10]);
    }

    #[test]
    fn goal_title_never_drops_below_sixty_percent() {
        let spec = [
            ColumnWidth::FillMin(60),
            ColumnWidth::Length(10),
            ColumnWidth::Length(10),
        ];
        // 40 total - 2 highlight - 2 spacing = 36 available; 60% of 36 = 21.
        assert_eq!(column_widths(40, &spec), vec![21, 10, 10]);
    }

    #[test]
    fn fill_columns_split_space_evenly() {
        let spec = [ColumnWidth::Fill, ColumnWidth::Fill, ColumnWidth::Fill];
        // 116 available -> 39, 39, 38.
        assert_eq!(column_widths(120, &spec), vec![39, 39, 38]);
    }

    #[test]
    fn skill_level_column_fits_the_full_bar() {
        let spec = [
            ColumnWidth::FillMin(60),
            ColumnWidth::Length(10),
            ColumnWidth::Length(16),
            ColumnWidth::Length(10),
        ];
        // 120 total - 2 highlight - 3 spacing = 115 available; name = 115 - 36.
        assert_eq!(column_widths(120, &spec), vec![79, 10, 16, 10]);
    }
}
