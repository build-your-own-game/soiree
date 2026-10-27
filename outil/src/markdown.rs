//! Just enough Markdown for the certification files: lines with their
//! number, fenced code blocks, HTML comments, table rows, identifiers.

/// One line of a file, numbered from 1.
#[derive(Clone, Copy, Debug)]
pub struct Line<'a> {
    pub number: usize,
    pub text: &'a str,
    /// Inside a fenced code block, fences included.
    pub fenced: bool,
    /// Inside an HTML comment.
    pub commented: bool,
}

impl Line<'_> {
    /// Neither code nor comment: what a reader of the rendered page sees.
    pub fn read(&self) -> bool {
        !self.fenced && !self.commented
    }
}

pub fn lines(text: &str) -> Vec<Line<'_>> {
    let mut fence: Option<&str> = None;
    let mut comment = false;
    let mut out = Vec::new();
    for (index, text) in text.lines().enumerate() {
        let trimmed = text.trim_start();
        let mut fenced = fence.is_some();
        let mut commented = comment;
        if let Some(open) = fence {
            if trimmed.starts_with(open) {
                fence = None;
            }
        } else if !comment {
            if trimmed.starts_with("```") {
                fence = Some("```");
                fenced = true;
            } else if trimmed.starts_with("~~~") {
                fence = Some("~~~");
                fenced = true;
            } else if let Some(start) = text.find("<!--") {
                commented = true;
                comment = !text.get(start..).is_some_and(|rest| rest.contains("-->"));
            }
        } else if text.contains("-->") {
            comment = false;
        }
        out.push(Line {
            number: index + 1,
            text,
            fenced,
            commented,
        });
    }
    out
}

/// The cells of a table row `| a | b |`, trimmed; `\|` stays inside a cell.
pub fn cells(row: &str) -> Vec<String> {
    let row = row.trim();
    let row = row.strip_prefix('|').unwrap_or(row);
    let row = row.strip_suffix('|').unwrap_or(row);
    let mut cells = vec![String::new()];
    let mut escaped = false;
    for c in row.chars() {
        if c == '|' && !escaped {
            cells.push(String::new());
        } else if let Some(cell) = cells.last_mut() {
            cell.push(c);
        }
        escaped = c == '\\';
    }
    cells.iter().map(|c| c.trim().to_owned()).collect()
}

/// `|---|:--:|` and the like.
pub fn is_separator(row: &str) -> bool {
    let cells = cells(row);
    !cells.is_empty()
        && cells.iter().all(|c| {
            let c = c.trim_matches(':');
            !c.is_empty() && c.chars().all(|x| x == '-')
        })
}

/// The text between backticks, in order.
pub fn code_spans(text: &str) -> Vec<&str> {
    text.split('`').skip(1).step_by(2).collect()
}

/// An identifier of the shared catalogue: 2 to 5 capitals, a dash, 2 digits
/// (`^[A-Z]{2,5}-\d{2}$`).
pub fn is_catalogue_id(id: &str) -> bool {
    let Some((prefix, number)) = id.split_once('-') else {
        return false;
    };
    (2..=5).contains(&prefix.len())
        && prefix.chars().all(|c| c.is_ascii_uppercase())
        && number.len() == 2
        && number.chars().all(|c| c.is_ascii_digit())
}

/// Every catalogue identifier written in `text`, as whole words.
pub fn catalogue_ids(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut found = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let boundary = start == 0 || chars.get(start - 1).is_none_or(|c| !c.is_alphanumeric());
        if boundary {
            for length in (5..=8).rev() {
                let Some(word) = chars.get(start..start + length) else {
                    continue;
                };
                let after = chars.get(start + length);
                let word: String = word.iter().collect();
                if is_catalogue_id(&word) && after.is_none_or(|c| !c.is_alphanumeric()) {
                    found.push(word);
                    start += length - 1;
                    break;
                }
            }
        }
        start += 1;
    }
    found
}

/// A Rust identifier.
pub fn is_ident(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c == '_' || c.is_ascii_alphabetic())
        && chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fences_and_comments_are_marked() {
        let text = "a\n```md\n| x |\n```\n<!-- c\nd -->\ne <!-- f --> g\nh";
        let read: Vec<usize> = lines(text)
            .iter()
            .filter(|l| l.read())
            .map(|l| l.number)
            .collect();
        assert_eq!(read, vec![1, 8]);
    }

    #[test]
    fn cells_and_separators() {
        assert_eq!(
            cells("| J1 | a \\| b | `t` | |"),
            vec!["J1", "a \\| b", "`t`", ""]
        );
        assert!(is_separator("|---|:--:|---|"));
        assert!(!is_separator("| # | Test |"));
    }

    #[test]
    fn catalogue_identifiers() {
        assert!(
            is_catalogue_id("PHY-01") && is_catalogue_id("VOTE-03") && is_catalogue_id("AB-99")
        );
        assert!(
            !is_catalogue_id("P-01") && !is_catalogue_id("PHYSIC-01") && !is_catalogue_id("PHY-1")
        );
        assert!(!is_catalogue_id("phy-01") && !is_catalogue_id("PHY-001"));
        assert_eq!(
            catalogue_ids("Voir PHY-01, VOTE-03 et (TXT-12) ; pas XPHY-012 ni aPHY-01."),
            vec!["PHY-01", "VOTE-03", "TXT-12"]
        );
    }
}
