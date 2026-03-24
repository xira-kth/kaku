use anstyle::{Color, Effects, RgbColor, Style};
use kaku_core::CodeFence;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme as SyntectTheme, ThemeSet};
use syntect::parsing::SyntaxSet;

use crate::{StyledSpan, Theme, layout::styled};

pub fn highlight_code(fence: &CodeFence, theme: Theme) -> Vec<Vec<StyledSpan>> {
    let syntax_set = SyntaxSet::load_defaults_newlines();
    let themes = ThemeSet::load_defaults();
    let syntax = fence
        .language
        .as_deref()
        .and_then(|name| syntax_set.find_syntax_by_token(name))
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text());
    let theme_name = if matches!(
        theme.heading,
        anstyle::AnsiColor::Blue | anstyle::AnsiColor::BrightBlue
    ) {
        "InspiredGitHub"
    } else {
        "base16-ocean.dark"
    };
    let selected = themes
        .themes
        .get(theme_name)
        .or_else(|| themes.themes.values().next())
        .expect("syntect ships with builtin themes");

    let mut highlighter = HighlightLines::new(syntax, selected);
    fence
        .code
        .lines()
        .map(|line| highlight_line(&mut highlighter, line, selected))
        .collect()
}

fn highlight_line(
    highlighter: &mut HighlightLines<'_>,
    line: &str,
    theme: &SyntectTheme,
) -> Vec<StyledSpan> {
    let Ok(ranges) = highlighter.highlight_line(line, &SyntaxSet::load_defaults_newlines()) else {
        return vec![styled(line.to_string(), fallback_style(theme), None)];
    };

    ranges
        .into_iter()
        .map(|(style, text)| {
            let mut ansi = Style::new().fg_color(Some(Color::Rgb(RgbColor(
                style.foreground.r,
                style.foreground.g,
                style.foreground.b,
            ))));
            if style
                .font_style
                .contains(syntect::highlighting::FontStyle::BOLD)
            {
                ansi = ansi.effects(Effects::BOLD);
            }
            if style
                .font_style
                .contains(syntect::highlighting::FontStyle::ITALIC)
            {
                ansi = ansi.effects(Effects::ITALIC);
            }
            styled(text.to_string(), ansi, None)
        })
        .collect()
}

fn fallback_style(_theme: &SyntectTheme) -> Style {
    Style::new()
}
