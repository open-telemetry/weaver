// SPDX-License-Identifier: Apache-2.0

//! Output formats supported by the `comment` filter.

use textwrap::core::Word;

pub mod html;
pub mod markdown;

fn is_ascii_space_or_newline(ch: char) -> bool {
    ch == ' ' || ch == '\r' || ch == '\n'
}

fn find_words_ascii_space_and_newline<'a>(line: &'a str) -> Box<dyn Iterator<Item = Word<'a>> + 'a> {
    let mut start = 0;
    let mut in_whitespace = false;
    let mut char_indices = line.char_indices();

    Box::new(std::iter::from_fn(move || {
        for (idx, ch) in char_indices.by_ref() {
            if in_whitespace && !is_ascii_space_or_newline(ch) {
                let word = Word::from(&line[start..idx].trim_end());
                start = idx;
                in_whitespace = is_ascii_space_or_newline(ch);
                return Some(word);
            }

            in_whitespace = is_ascii_space_or_newline(ch);
        }

        if start < line.len() {
            let word = Word::from(&line[start..].trim_end());
            start = line.len();
            return Some(word);
        }

        None
    }))
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use super::find_words_ascii_space_and_newline;


    fn find_words_into_vec(line: &str) -> Vec<String> {
        find_words_ascii_space_and_newline(line)
            .map(|w| w.to_string())
            .collect_vec()
    }

    #[test]
    fn test_find_words_dont_split_markdown() {
        assert_eq!(find_words_into_vec("test\nthe words"),
    vec!("test", "the", "words"));
    }
}