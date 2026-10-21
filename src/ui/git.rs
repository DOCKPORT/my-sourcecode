use crate::scan::GitStats;
use crate::ui::format::commas;
use crate::ui::table::{TableSpec, draw_table};
use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::widgets::{Block, Borders};

/// The read-only repository information shown under the language summary.
pub struct GitSummary {
    stats: GitStats,
}

impl GitSummary {
    /// Build the table from the collected repository information.
    pub fn new(stats: GitStats) -> Self {
        Self { stats }
    }

    /// Draw the Git table into the given area.
    pub fn draw(&self, frame: &mut Frame<'_>, area: Rect) {
        let header = [
            "branch",
            "commits",
            "tags",
            "latest tag",
            "first commit",
            "last commit",
        ]
        .map(str::to_string)
        .to_vec();
        let row = vec![
            self.stats.branch.clone(),
            commas(self.stats.commits),
            commas(self.stats.tags),
            self.stats.latest_tag.clone(),
            self.stats.first_commit.clone(),
            self.stats.last_commit.clone(),
        ];

        let widths = [Constraint::Ratio(1, 6); 6];
        let centered = [false; 6];
        let block = Block::default().borders(Borders::ALL).title(" Git ");

        draw_table(
            frame,
            area,
            TableSpec {
                block,
                header,
                rows: vec![row],
                widths: widths.to_vec(),
                left_aligned: centered.to_vec(),
                row_rules: false,
                body_dividers: true,
                color_column: None,
                cell_colors: Vec::new(),
            },
            None,
        );
    }
}
