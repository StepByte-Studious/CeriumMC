//! Line diff for readable parity reports.

/// One line of a diff between the vanilla and CeriumMC outputs.
#[derive(Debug, PartialEq, Eq)]
pub enum Line<'a> {
    Same(&'a str),
    Vanilla(&'a str),
    Cerium(&'a str),
}

/// Diffs two texts line by line, using a longest common subsequence.
pub fn diff<'a>(vanilla: &'a str, cerium: &'a str) -> Vec<Line<'a>> {
    let a: Vec<&str> = vanilla.lines().collect();
    let b: Vec<&str> = cerium.lines().collect();

    // lcs[i][j] is the length of the longest common subsequence of
    // a[i..] and b[j..].
    let mut lcs = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }

    let mut out = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            out.push(Line::Same(a[i]));
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            out.push(Line::Vanilla(a[i]));
            i += 1;
        } else {
            out.push(Line::Cerium(b[j]));
            j += 1;
        }
    }
    out.extend(a[i..].iter().copied().map(Line::Vanilla));
    out.extend(b[j..].iter().copied().map(Line::Cerium));
    out
}

#[cfg(test)]
mod tests {
    use super::{Line, diff};

    #[test]
    fn identical_texts_have_no_changes() {
        let lines = diff("a\nb", "a\nb");
        assert_eq!(lines, [Line::Same("a"), Line::Same("b")]);
    }

    #[test]
    fn changed_line_is_reported_on_both_sides() {
        let lines = diff("a\nb\nc", "a\nx\nc");
        let expected = [
            Line::Same("a"),
            Line::Vanilla("b"),
            Line::Cerium("x"),
            Line::Same("c"),
        ];
        assert_eq!(lines, expected);
    }

    #[test]
    fn missing_and_extra_lines() {
        assert_eq!(diff("a", ""), [Line::Vanilla("a")]);
        assert_eq!(diff("", "a"), [Line::Cerium("a")]);
    }
}
