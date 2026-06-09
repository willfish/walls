use super::style::{self, StateKind};

pub(super) fn lines(logs: &[String], cursor: usize, width: u16, height: u16) -> Vec<String> {
    if logs.is_empty() {
        return vec![style::state_text(StateKind::Empty, "no logs captured yet")];
    }
    let wrap_width = usize::from(width).saturating_sub(4);
    let mut lines = Vec::new();
    let mut selected_row = 0;
    for (i, line) in logs.iter().rev().enumerate() {
        let selected = i == cursor;
        let mark = if selected { ">" } else { " " };
        let wrapped = wrap_text(line, wrap_width);
        for (j, segment) in wrapped.into_iter().enumerate() {
            if j == 0 {
                if selected {
                    selected_row = lines.len();
                }
                lines.push(format!("{mark} {segment}"));
            } else {
                lines.push(format!("  {segment}"));
            }
        }
    }
    crop_around_selection(lines, selected_row, height)
}

fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }
    if text.len() <= width {
        return vec![text.to_string()];
    }

    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let extra = if current.is_empty() {
            word.len()
        } else {
            current.len() + 1 + word.len()
        };
        if current.is_empty() {
            current = word.to_string();
        } else if extra <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(current);
            current = word.to_string();
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(text.to_string());
    }
    lines
}

fn crop_around_selection(
    lines: Vec<String>,
    selected_row: usize,
    viewport_height: u16,
) -> Vec<String> {
    let visible_rows = usize::from(viewport_height).saturating_sub(2).max(1);
    if lines.len() <= visible_rows {
        return lines;
    }
    let start = selected_row
        .saturating_add(1)
        .saturating_sub(visible_rows)
        .min(lines.len().saturating_sub(visible_rows));
    lines.into_iter().skip(start).take(visible_rows).collect()
}

#[cfg(test)]
mod tests {
    use super::lines;

    #[test]
    fn lines_render_newest_first_with_cursor_marker() {
        let logs = ["oldest", "middle", "newest"].map(str::to_string);

        let rendered = lines(&logs, 1, 80, 12);

        assert_eq!(rendered[0], "  newest");
        assert_eq!(rendered[1], "> middle");
        assert_eq!(rendered[2], "  oldest");
    }

    #[test]
    fn lines_keep_wrapped_selected_row_visible_when_cropped() {
        let logs = [
            "oldest line",
            "older selected line wraps across several visual rows",
            "middle line",
            "newest line",
        ]
        .map(str::to_string);

        let rendered = lines(&logs, 2, 22, 5);

        assert!(
            rendered
                .iter()
                .any(|line| line.starts_with("> ") && line.contains("older selected")),
            "{rendered:?}"
        );
        assert!(rendered.len() <= 3, "{rendered:?}");
    }
}
