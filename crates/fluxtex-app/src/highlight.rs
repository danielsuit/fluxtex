use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

use floem::peniko::Color;
use floem::text::{Attrs, AttrsList, FamilyOwned, Stretch, Style as FontStyle, Weight};
use floem::views::editor::core::buffer::rope_text::RopeText;
use floem::views::editor::{id::EditorId, text::Styling, Editor, EditorStyle};

#[derive(Clone, Copy)]
pub struct LatexThemeColors {
    pub comment: Color,
    pub command: Color,
    pub environment: Color,
    pub math: Color,
    pub delimiter: Color,
}

pub type EditorHandle = Rc<RefCell<Option<Editor>>>;

pub struct LatexStyling {
    editor: EditorHandle,
    font_family: Vec<FamilyOwned>,
    palette: LatexThemeColors,
    theme_id: u64,
}

pub fn latex_styling(
    editor: EditorHandle,
    theme_id: u64,
    palette: LatexThemeColors,
) -> LatexStyling {
    LatexStyling {
        editor,
        font_family: vec![FamilyOwned::Name("JetBrains Mono".to_string())],
        palette,
        theme_id,
    }
}

impl Styling for LatexStyling {
    fn id(&self) -> u64 {
        self.theme_id
    }

    fn font_size(&self, _edid: EditorId, _line: usize) -> usize {
        14
    }

    fn line_height(&self, _edid: EditorId, _line: usize) -> f32 {
        22.0
    }

    fn font_family(&self, _edid: EditorId, _line: usize) -> Cow<'_, [FamilyOwned]> {
        Cow::Borrowed(&self.font_family)
    }

    fn weight(&self, _edid: EditorId, _line: usize) -> Weight {
        Weight::NORMAL
    }

    fn italic_style(&self, _edid: EditorId, _line: usize) -> FontStyle {
        FontStyle::Normal
    }

    fn stretch(&self, _edid: EditorId, _line: usize) -> Stretch {
        Stretch::Normal
    }

    fn apply_attr_styles(
        &self,
        _edid: EditorId,
        _style: &EditorStyle,
        line: usize,
        default: Attrs,
        attrs: &mut AttrsList,
    ) {
        let editor_ref = self.editor.borrow();
        let Some(editor) = editor_ref.as_ref() else {
            return;
        };

        let rope_text = editor.rope_text();
        if line > rope_text.last_line() {
            return;
        }

        let raw: Cow<str> = rope_text.line_content(line);
        let trimmed: &str = raw
            .strip_suffix("\r\n")
            .or_else(|| raw.strip_suffix('\n'))
            .unwrap_or(&raw);

        if trimmed.is_empty() {
            return;
        }

        let parse_until = comment_start(trimmed).unwrap_or(trimmed.len());

        highlight_commands(trimmed, parse_until, default, attrs, self.palette);
        highlight_math(trimmed, parse_until, default, attrs, self.palette);
        highlight_delimiters(trimmed, parse_until, default, attrs, self.palette);

        if let Some(start) = comment_start(trimmed) {
            attrs.add_span(
                start..trimmed.len(),
                default.color(self.palette.comment).style(FontStyle::Italic),
            );
        }
    }
}

fn highlight_commands(
    line: &str,
    limit: usize,
    default: Attrs,
    attrs: &mut AttrsList,
    palette: LatexThemeColors,
) {
    let bytes = line.as_bytes();
    let mut index = 0;

    while index < limit {
        if bytes[index] != b'\\' {
            index += 1;
            continue;
        }

        let command_end = command_end(line, index, limit);
        if command_end <= index + 1 {
            index += 1;
            continue;
        }

        attrs.add_span(index..command_end, default.color(palette.command));

        let command = &line[index + 1..command_end];
        if matches!(command, "begin" | "end") {
            let mut cursor = command_end;
            while cursor < limit && bytes[cursor].is_ascii_whitespace() {
                cursor += 1;
            }

            if cursor < limit && bytes[cursor] == b'{' {
                if let Some(close) = line[cursor + 1..limit].find('}') {
                    let end = cursor + 1 + close;
                    if cursor + 1 < end {
                        attrs.add_span(cursor + 1..end, default.color(palette.environment));
                    }
                }
            }
        }

        index = command_end;
    }
}

fn highlight_math(
    line: &str,
    limit: usize,
    default: Attrs,
    attrs: &mut AttrsList,
    palette: LatexThemeColors,
) {
    let bytes = line.as_bytes();
    let mut index = 0;
    let mut open_start = None;
    let mut open_len = 0;

    while index < limit {
        if bytes[index] == b'$' && !is_escaped(bytes, index) {
            let delim_len = if index + 1 < limit && bytes[index + 1] == b'$' {
                2
            } else {
                1
            };

            if let Some(start) = open_start {
                if open_len == delim_len {
                    attrs.add_span(start..(index + delim_len), default.color(palette.math));
                    open_start = None;
                    open_len = 0;
                    index += delim_len;
                    continue;
                }
            } else {
                open_start = Some(index);
                open_len = delim_len;
                index += delim_len;
                continue;
            }
        }

        index += 1;
    }

    if let Some(start) = open_start {
        attrs.add_span(start..limit, default.color(palette.math));
    }
}

fn highlight_delimiters(
    line: &str,
    limit: usize,
    default: Attrs,
    attrs: &mut AttrsList,
    palette: LatexThemeColors,
) {
    for (index, ch) in line.char_indices() {
        if index >= limit {
            break;
        }

        if matches!(ch, '{' | '}' | '[' | ']' | '(' | ')') {
            attrs.add_span(
                index..index + ch.len_utf8(),
                default.color(palette.delimiter),
            );
        }
    }
}

fn comment_start(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'%' && !is_escaped(bytes, index) {
            return Some(index);
        }
    }
    None
}

fn command_end(line: &str, start: usize, limit: usize) -> usize {
    let bytes = line.as_bytes();
    if start + 1 >= limit {
        return start + 1;
    }

    let next = bytes[start + 1];
    if next.is_ascii_alphabetic() || next == b'@' {
        let mut index = start + 2;
        while index < limit && (bytes[index].is_ascii_alphabetic() || bytes[index] == b'@') {
            index += 1;
        }
        index
    } else {
        (start + 2).min(limit)
    }
}

fn is_escaped(bytes: &[u8], index: usize) -> bool {
    let mut backslashes = 0;
    let mut cursor = index;

    while cursor > 0 {
        cursor -= 1;
        if bytes[cursor] == b'\\' {
            backslashes += 1;
        } else {
            break;
        }
    }

    backslashes % 2 == 1
}
