use crate::ui::theme;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Text;
use ratatui::widgets::{Block, Cell, Paragraph, Row, Table, TableState};

/// Space between columns in every table.
const COLUMN_SPACING: u16 = 2;
/// The horizontal line drawn below a header row.
const HEADER_RULE: &str = "─";
/// The vertical line drawn between columns.
const DIVIDER: &str = "│";
/// Width of the `> ` selection marker.
const SELECTION_WIDTH: u16 = 2;

/// The content and options for one table.
pub struct TableSpec<'a> {
    pub block: Block<'a>,
    pub header: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub widths: Vec<Constraint>,
    pub left_aligned: Vec<bool>,
    pub row_rules: bool,
    pub body_dividers: bool,
    /// Optional per-row color for one column.
    pub color_column: Option<usize>,
    pub cell_colors: Vec<Option<Color>>,
}

/// Draw a table in three parts: the header, a subtle rule, and the body.
///
/// The helper computes the column widths once. It uses them for text padding,
/// for the header and body widgets, and for the vertical dividers. Header
/// labels and all values are centered, except columns marked as left aligned.
///
/// An interactive table passes a state. The header then reserves the same space
/// for the highlight symbol, and the body keeps its scroll offset.
pub fn draw_table<'a>(
    frame: &mut Frame<'_>,
    area: Rect,
    spec: TableSpec<'a>,
    state: Option<&mut TableState>,
) {
    let TableSpec {
        block,
        header,
        rows,
        widths,
        left_aligned,
        row_rules,
        body_dividers,
        color_column,
        cell_colors,
    } = spec;

    let block = block
        .style(Style::default().bg(theme::BACKGROUND).fg(theme::FOREGROUND))
        .border_style(Style::default().fg(theme::BORDER))
        .title_style(Style::default().fg(theme::FOREGROUND));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 || widths.is_empty() {
        return;
    }

    let interactive = state.is_some();
    let selection_width = if interactive { SELECTION_WIDTH } else { 0 };
    let columns = column_areas(inner, &widths, selection_width);
    let column_widths: Vec<u16> = columns.iter().map(|column| column.width).collect();

    let header_cells: Vec<Cell> = header
        .iter()
        .enumerate()
        .map(|(index, label)| {
            let width = column_widths.get(index).copied().unwrap_or(0);
            Cell::from(Text::raw(pad(label, width, false)))
        })
        .collect();
    let header_row = Row::new(header_cells).style(header_style());

    let header_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    };
    let mut header_table = Table::new(Vec::<Row<'a>>::new(), widths.clone())
        .header(header_row)
        .style(Style::default().fg(theme::FOREGROUND).bg(theme::BACKGROUND))
        .column_spacing(COLUMN_SPACING);
    if interactive {
        // Reserve the highlight symbol width in the header so that the header
        // columns line up with the body columns. The temporary selection does
        // not style the header; it only reserves the space.
        header_table = header_table.highlight_symbol("> ");
        let mut header_state = TableState::default();
        header_state.select(Some(0));
        frame.render_stateful_widget(header_table, header_area, &mut header_state);
    } else {
        frame.render_widget(header_table, header_area);
    }

    if inner.height >= 2 {
        let rule_area = Rect {
            x: inner.x,
            y: inner.y + 1,
            width: inner.width,
            height: 1,
        };
        let rule = Paragraph::new(Text::raw(HEADER_RULE.repeat(inner.width as usize)))
            .style(Style::default().fg(theme::MUTED).bg(theme::BACKGROUND));
        frame.render_widget(rule, rule_area);
    }

    if inner.height >= 3 {
        let body_area = Rect {
            x: inner.x,
            y: inner.y + 2,
            width: inner.width,
            height: inner.height - 2,
        };
        let body_rows: Vec<Row> = rows
            .iter()
            .enumerate()
            .map(|(row_index, row)| {
                let cells: Vec<Cell> = row
                    .iter()
                    .enumerate()
                    .map(|(index, value)| {
                        let width = column_widths.get(index).copied().unwrap_or(0);
                        let left = left_aligned.get(index).copied().unwrap_or(false);
                        let mut cell = Cell::from(Text::raw(pad(value, width, left)));
                        if color_column == Some(index)
                            && let Some(Some(color)) = cell_colors.get(row_index)
                        {
                            cell = cell.style(Style::default().fg(*color));
                        }
                        cell
                    })
                    .collect();
                Row::new(cells)
            })
            .collect();
        let table = Table::new(body_rows, widths.clone())
            .style(Style::default().fg(theme::FOREGROUND).bg(theme::BACKGROUND))
            .column_spacing(COLUMN_SPACING);

        let draw_rules = row_rules && state.is_none();
        match state {
            Some(state) => {
                let table = table
                    .highlight_symbol("> ")
                    .highlight_style(highlight_style());
                frame.render_stateful_widget(table, body_area, state);
            }
            None => frame.render_widget(table, body_area),
        }

        if draw_rules {
            draw_row_rules(frame, body_area, &rows);
        }
    }

    draw_dividers(frame, inner, &columns, body_dividers);
}

/// Compute the column areas with the same rules the table widget uses.
fn column_areas(inner: Rect, widths: &[Constraint], selection_width: u16) -> Vec<Rect> {
    let columns_width = inner.width.saturating_sub(selection_width);
    if columns_width == 0 {
        return Vec::new();
    }
    let area = Rect {
        x: inner.x + selection_width,
        y: inner.y,
        width: columns_width,
        height: 1,
    };
    Layout::horizontal(widths.to_vec())
        .spacing(COLUMN_SPACING)
        .split(area)
        .to_vec()
}

/// Draw a horizontal rule on each all-empty separator row.
///
/// The languages table reserves one blank row between entries. This function
/// fills those rows with the same DarkGray line that sits under the header.
fn draw_row_rules(frame: &mut Frame<'_>, body_area: Rect, rows: &[Vec<String>]) {
    for (index, row) in rows.iter().enumerate() {
        if !row.iter().all(|value| value.is_empty()) {
            continue;
        }
        let y = body_area.y.saturating_add(index as u16);
        if y >= body_area.bottom() {
            break;
        }
        let area = Rect {
            x: body_area.x,
            y,
            width: body_area.width,
            height: 1,
        };
        let line = Paragraph::new(Text::raw(HEADER_RULE.repeat(area.width as usize)))
            .style(Style::default().fg(theme::MUTED).bg(theme::BACKGROUND));
        frame.render_widget(line, area);
    }
}

/// Draw a vertical line in each gap between columns.
///
/// The lines use the same DarkGray color as the header rule. The positions
/// come from the computed column areas, so the lines stay between columns.
/// When `body_dividers` is false, the lines cover only the header row.
fn draw_dividers(frame: &mut Frame<'_>, inner: Rect, columns: &[Rect], body_dividers: bool) {
    if columns.len() < 2 {
        return;
    }

    let bottom = if body_dividers {
        inner.bottom()
    } else {
        inner.top().saturating_add(1).min(inner.bottom())
    };

    let buffer = frame.buffer_mut();
    for column in columns.iter().take(columns.len() - 1) {
        let x = column.right();
        if x >= inner.right() {
            continue;
        }
        for y in inner.top()..bottom {
            let cell = buffer.get_mut(x, y);
            cell.set_symbol(DIVIDER);
            cell.set_fg(theme::MUTED);
        }
    }
}

/// Pad text to a column width. Center it unless `left` is true.
fn pad(text: &str, width: u16, left: bool) -> String {
    let width = width as usize;
    let length = text.chars().count();
    if length >= width {
        return text.to_string();
    }

    let spaces = width - length;
    if left {
        return format!("{text}{}", " ".repeat(spaces));
    }

    let before = spaces / 2;
    let after = spaces - before;
    format!("{}{}{}", " ".repeat(before), text, " ".repeat(after))
}

/// Style for every header row.
fn header_style() -> Style {
    Style::default()
        .fg(theme::HEADER)
        .bg(theme::BACKGROUND)
        .add_modifier(Modifier::BOLD)
}

/// Style for the selected file row.
fn highlight_style() -> Style {
    Style::default()
        .fg(theme::SELECTED_FG)
        .bg(theme::SELECTED_BG)
        .add_modifier(Modifier::BOLD)
}
