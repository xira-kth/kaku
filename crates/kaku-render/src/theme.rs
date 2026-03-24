use anstyle::AnsiColor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeName {
    Auto,
    Light,
    Dark,
    Ansi,
}

impl ThemeName {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "auto" => Some(Self::Auto),
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            "ansi" => Some(Self::Ansi),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub heading: AnsiColor,
    pub link: AnsiColor,
    pub code: AnsiColor,
    pub quote: AnsiColor,
    pub rule: AnsiColor,
    pub muted: AnsiColor,
    pub accent: AnsiColor,
}

impl Theme {
    pub fn resolve(name: ThemeName) -> Self {
        match name {
            ThemeName::Auto | ThemeName::Dark => Self {
                heading: AnsiColor::Cyan,
                link: AnsiColor::Blue,
                code: AnsiColor::Green,
                quote: AnsiColor::BrightBlack,
                rule: AnsiColor::BrightBlack,
                muted: AnsiColor::BrightBlack,
                accent: AnsiColor::Yellow,
            },
            ThemeName::Light => Self {
                heading: AnsiColor::Blue,
                link: AnsiColor::BrightBlue,
                code: AnsiColor::Magenta,
                quote: AnsiColor::BrightBlack,
                rule: AnsiColor::BrightBlack,
                muted: AnsiColor::BrightBlack,
                accent: AnsiColor::Red,
            },
            ThemeName::Ansi => Self {
                heading: AnsiColor::Yellow,
                link: AnsiColor::Cyan,
                code: AnsiColor::Green,
                quote: AnsiColor::White,
                rule: AnsiColor::White,
                muted: AnsiColor::White,
                accent: AnsiColor::Magenta,
            },
        }
    }
}
