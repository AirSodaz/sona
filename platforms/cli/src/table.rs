use unicode_width::UnicodeWidthStr;

pub(crate) fn sanitize_table_cell(value: &str) -> String {
    let mut sanitized = String::with_capacity(value.len());
    for character in value.chars() {
        if character.is_control() {
            sanitized.extend(character.escape_default());
        } else {
            sanitized.push(character);
        }
    }
    sanitized
}

pub(crate) fn column_widths<const N: usize>(
    headers: &[&str; N],
    rows: &[[String; N]],
) -> [usize; N] {
    let mut widths = std::array::from_fn(|index| UnicodeWidthStr::width(headers[index]));
    for row in rows {
        for (index, value) in row.iter().enumerate() {
            widths[index] = widths[index].max(UnicodeWidthStr::width(value.as_str()));
        }
    }
    widths
}

pub(crate) fn append_table_row<const N: usize>(
    output: &mut String,
    values: &[&str; N],
    widths: &[usize; N],
) {
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            output.push_str("  ");
        }
        output.push_str(value);
        output.push_str(&" ".repeat(widths[index].saturating_sub(UnicodeWidthStr::width(*value))));
    }
    output.push('\n');
}

pub(crate) fn append_table_separator<const N: usize>(output: &mut String, widths: &[usize; N]) {
    for (index, width) in widths.iter().enumerate() {
        if index > 0 {
            output.push_str("  ");
        }
        output.push_str(&"-".repeat(*width));
    }
    output.push('\n');
}

#[cfg(test)]
mod tests {
    use super::{append_table_row, column_widths, sanitize_table_cell};
    use unicode_width::UnicodeWidthStr;

    #[test]
    fn sanitizes_control_characters_without_changing_printable_text() {
        assert_eq!(
            sanitize_table_cell("row\nname\t\u{1b}\u{7}会议"),
            r"row\nname\t\u{1b}\u{7}会议"
        );
    }

    #[test]
    fn aligns_unicode_columns_by_display_width() {
        let headers = ["NAME", "STATUS"];
        let rows = [
            ["会议规则".to_string(), "ready".to_string()],
            ["meeting1".to_string(), "ready".to_string()],
        ];
        let widths = column_widths(&headers, &rows);
        let mut output = String::new();

        for row in &rows {
            let values = [row[0].as_str(), row[1].as_str()];
            append_table_row(&mut output, &values, &widths);
        }

        let status_columns = output
            .lines()
            .map(|line| UnicodeWidthStr::width(&line[..line.find("ready").unwrap()]))
            .collect::<Vec<_>>();
        assert_eq!(status_columns, vec![10, 10]);
    }
}
