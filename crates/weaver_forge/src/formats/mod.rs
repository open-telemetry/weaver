// SPDX-License-Identifier: Apache-2.0

//! Output formats supported by the `comment` filter.

use std::fmt::Write;

use crate::config::default_bool;
use serde::{Deserialize, Serialize};
use textwrap::{core::Word, WordSeparator};

pub mod html;
pub mod markdown;

/// Configuration for word-wrap behavior.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct WordWrapConfig {
    /// The limit of characters per-line.
    /// If this contains a size, word wrapping will ensure
    /// lines do not exceed this size UNLESS un-splittable
    /// words exceed this size.
    pub(crate) line_length: Option<usize>,
    /// True if we wrap words across newlines and don't preserve them.
    /// Note: This behavior is nuanced and likely needs finer grained control.
    #[serde(default = "default_bool::<false>")]
    pub(crate) ignore_newlines: bool,
}

impl WordWrapConfig {
    // Applies an override to the config.
    fn with_line_length_override(&self, line_length_override: Option<usize>) -> Self {
        match line_length_override {
            Some(size) => WordWrapConfig {
                line_length: Some(size),
                ignore_newlines: self.ignore_newlines,
            },
            None => self.clone(),
        }
    }
}

fn is_ascii_space_or_newline(ch: char) -> bool {
    ch == ' ' || ch == '\r' || ch == '\n'
}

fn find_words_ascii_space_and_newline<'a>(
    line: &'a str,
) -> Box<dyn Iterator<Item = Word<'a>> + 'a> {
    let mut start = 0;
    let mut in_whitespace = false;
    let mut char_indices = line.char_indices();

    Box::new(std::iter::from_fn(move || {
        for (idx, ch) in char_indices.by_ref() {
            if in_whitespace && !is_ascii_space_or_newline(ch) {
                let word = Word::from(line[start..idx].trim_end());
                start = idx;
                in_whitespace = is_ascii_space_or_newline(ch);
                return Some(word);
            }

            in_whitespace = is_ascii_space_or_newline(ch);
        }

        if start < line.len() {
            let word = Word::from(line[start..].trim_end());
            start = line.len();
            return Some(word);
        }

        None
    }))
}

struct WordWrapContext {
    // Mechanism we use to split words.
    word_separator: WordSeparator,
    // The limit of characters per-line.
    line_length: Option<usize>,

    // Current length of a line being rendered.
    current_line_length: usize,

    // True if there's a dangling space from previously written
    // word we may choose to ignore.
    letfover_space: bool,

    // True if we wrap across newlines and don't preserve them.
    ignore_newlines: bool,
}

fn default_word_separator(ignore_newlines: bool) -> WordSeparator {
    if ignore_newlines {
        WordSeparator::Custom(find_words_ascii_space_and_newline)
    } else {
        WordSeparator::AsciiSpace
    }
}

impl WordWrapContext {
    fn new(cfg: &WordWrapConfig) -> Self {
        Self {
            word_separator: default_word_separator(cfg.ignore_newlines),
            line_length: cfg.line_length,
            current_line_length: Default::default(),
            letfover_space: Default::default(),
            ignore_newlines: cfg.ignore_newlines,
        }
    }

    fn write_unbroken<O: Write>(
        &mut self,
        out: &mut O,
        input: &str,
        indent: &str,
    ) -> std::fmt::Result {
        if self
            .line_length
            .map(|max| self.current_line_length + input.len() > max)
            .unwrap_or(false)
        {
            write!(out, "\n{indent}")?;
            self.current_line_length = indent.len();
        } else if self.letfover_space {
            write!(out, " ")?;
            self.current_line_length += 1;
        }
        write!(out, "{input}")?;
        self.current_line_length += input.len();
        self.letfover_space = false;
        Ok(())
    }
    fn write_ln<O: Write>(&mut self, out: &mut O, indent: &str) -> std::fmt::Result {
        write!(out, "\n{indent}")?;
        self.current_line_length = indent.len();
        self.letfover_space = false;
        Ok(())
    }
    fn write_words<O: Write>(
        &mut self,
        out: &mut O,
        input: &str,
        indent: &str,
    ) -> std::fmt::Result {
        // Just push the words directly if no limits.
        if self.line_length.is_none() {
            write!(out, "{input}")?;
            self.current_line_length += input.len();
            return Ok(());
        }
        let mut first = true;
        for word in self.word_separator.find_words(input) {
            // We either add an end of line or space between words.
            let mut newline = false;
            if self
                .line_length
                .map(|max| self.current_line_length + word.len() > max)
                .unwrap_or(false)
            {
                // Split the line.
                write!(out, "\n{indent}")?;
                self.current_line_length = indent.len();
                newline = true;
            } else if self.letfover_space || !first {
                write!(out, " ")?;
                self.current_line_length += 1;
            }
            // Handle a scenario where we created a new line
            // and don't want a space in it.
            if first && newline {
                write!(out, "{}", word.trim_start())?;
                self.current_line_length += word.trim_start().len();
            } else {
                write!(out, "{}", word.word)?;
                self.current_line_length += word.len();
            }

            first = false;
            self.letfover_space = false;
        }
        // TODO - mark this as tailing so we can later decide to add it or
        // newline.
        // We struggle with the AST of markdown here.
        self.letfover_space =
            input.ends_with(' ') || (self.ignore_newlines && input.ends_with('\n'));
        Ok(())
    }

    fn write_unbroken_ln<O: Write>(
        &mut self,
        out: &mut O,
        input: &str,
        indent: &str,
    ) -> std::fmt::Result {
        self.write_unbroken(out, input, indent)?;
        self.write_ln(out, indent)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::find_words_ascii_space_and_newline;
    use itertools::Itertools;

    fn find_words_into_vec(line: &str) -> Vec<String> {
        find_words_ascii_space_and_newline(line)
            .map(|w| w.to_string())
            .collect_vec()
    }

    #[test]
    fn test_find_words_dont_split_markdown() {
        assert_eq!(
            find_words_into_vec("test\nthe words"),
            vec!("test", "the", "words")
        );
    }
}
