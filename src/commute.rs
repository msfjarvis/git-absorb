extern crate failure;

use owned;

/// Returns the unchanged lines around this hunk.
///
/// Any given hunk has four anchor points:
///
/// - the last unchanged line before it, on the removed side
/// - the first unchanged line after it, on the removed side
/// - the last unchanged line before it, on the added side
/// - the first unchanged line after it, on the added side
///
/// This function returns those four line numbers, in that order.
fn anchors(hunk: &owned::Hunk) -> (usize, usize, usize, usize) {
    match (hunk.removed.lines.len(), hunk.added.lines.len()) {
        (0, 0) => (0, 1, 0, 1),
        (removed_len, 0) => (
            hunk.removed.start - 1,
            hunk.removed.start + removed_len,
            hunk.removed.start - 1,
            hunk.removed.start,
        ),
        (0, added_len) => (
            hunk.added.start - 1,
            hunk.added.start,
            hunk.added.start - 1,
            hunk.added.start + added_len,
        ),
        (removed_len, added_len) => (
            hunk.removed.start - 1,
            hunk.removed.start + removed_len,
            hunk.added.start - 1,
            hunk.added.start + added_len,
        ),
    }
}

/// Tests if all elements of the iterator are equal to each other.
///
/// An empty iterator returns `true`.
///
/// `uniform()` is short-circuiting. It will stop processing as soon
/// as it finds two pairwise inequal elements.
fn uniform<I, E>(mut iter: I) -> bool
where
    I: ::std::iter::Iterator<Item = E>,
    E: ::std::cmp::Eq,
{
    match iter.next() {
        Some(first) => iter.all(|e| e == first),
        None => true,
    }
}

fn commute(
    first: &owned::Hunk,
    second: &owned::Hunk,
) -> Result<Option<(owned::Hunk, owned::Hunk)>, failure::Error> {
    // represent hunks in content order rather than application order
    let (first_above, above, below) = match (
        // TODO: skip any comparisons against empty blocks
        first.added.start <= second.added.start,
        first.removed.start <= second.removed.start,
    ) {
        (true, true) => (true, first, second),
        (false, false) => (false, second, first),
        _ => return Err(failure::err_msg("nonsensical hunk ordering")),
    };

    // if both hunks are exclusively adding or removing, and both
    // hunks are composed entirely of the same line being repeated,
    // then they commute no matter what their offsets are, because
    // they can be interleaved in any order without changing the final
    // result
    let interleavable = {
        if above.added.lines.is_empty() && below.added.lines.is_empty() {
            uniform(above.removed.lines.iter().chain(&*below.removed.lines))
        } else if above.removed.lines.is_empty() && below.removed.lines.is_empty() {
            uniform(above.added.lines.iter().chain(&*below.added.lines))
        } else {
            false
        }
    };
    // there has to be at least one unchanged line between the two
    // hunks on the first hunk's added side, and the second hunk's
    // removed side
    let (above_anchor, below_anchor) = if first_above {
        (anchors(above).3, anchors(below).0)
    } else {
        (anchors(above).1, anchors(below).2)
    };
    // the hunks overlap and are not interleavable, so they cannot
    // commute
    if above_anchor > below_anchor && !interleavable {
        return Ok(None);
    }

    let above = above.clone();
    let mut below = below.clone();
    let above_change_offset = (above.added.lines.len() as i64 - above.removed.lines.len() as i64)
        * if first_above { -1 } else { 1 };
    below.added.start = (below.added.start as i64 + above_change_offset) as usize;
    below.removed.start = (below.removed.start as i64 + above_change_offset) as usize;

    Ok(Some(if first_above {
        (below, above)
    } else {
        (above, below)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    #[test]
    fn test_commute() {
        let hunk1 = owned::Hunk {
            added: owned::Block {
                start: 2,
                lines: Rc::new(vec![b"bar\n".to_vec()]),
                trailing_newline: true,
            },
            removed: owned::Block {
                start: 1,
                lines: Rc::new(vec![]),
                trailing_newline: true,
            },
        };

        let hunk2 = owned::Hunk {
            added: owned::Block {
                start: 1,
                lines: Rc::new(vec![b"bar\n".to_vec()]),
                trailing_newline: true,
            },
            removed: owned::Block {
                start: 0,
                lines: Rc::new(vec![]),
                trailing_newline: true,
            },
        };

        let (new1, new2) = commute(&hunk1, &hunk2).unwrap().unwrap();
        assert_eq!(new1.added.start, 1);
        assert_eq!(new2.added.start, 3);
    }

    #[test]
    fn test_commute_interleave() {
        let mut line = ::std::iter::repeat(b"bar\n".to_vec());
        let hunk1 = owned::Hunk {
            added: owned::Block {
                start: 1,
                lines: Rc::new((&mut line).take(4).collect::<Vec<_>>()),
                trailing_newline: true,
            },
            removed: owned::Block {
                start: 0,
                lines: Rc::new(vec![]),
                trailing_newline: true,
            },
        };
        let hunk2 = owned::Hunk {
            added: owned::Block {
                start: 1,
                lines: Rc::new((&mut line).take(2).collect::<Vec<_>>()),
                trailing_newline: true,
            },
            removed: owned::Block {
                start: 0,
                lines: Rc::new(vec![]),
                trailing_newline: true,
            },
        };

        let (new1, new2) = commute(&hunk1, &hunk2).unwrap().unwrap();
        assert_eq!(new1.added.lines.len(), 2);
        assert_eq!(new2.added.lines.len(), 4);
    }
}