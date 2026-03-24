use anstyle::AnsiColor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeName {
    Auto,
    Light,
    Dark,
    Minimal,
}

impl ThemeName {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "auto" => Some(Self::Auto),
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            "minimal" | "ansi" => Some(Self::Minimal),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub monochrome: bool,
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
            ThemeName::Auto => Self {
                monochrome: true,
                heading: AnsiColor::White,
                link: AnsiColor::BrightBlack,
                code: AnsiColor::BrightBlack,
                quote: AnsiColor::BrightBlack,
                rule: AnsiColor::BrightBlack,
                muted: AnsiColor::BrightBlack,
                accent: AnsiColor::BrightBlack,
            },
            ThemeName::Dark => Self {
                monochrome: false,
                heading: AnsiColor::White,
                link: AnsiColor::BrightBlack,
                code: AnsiColor::BrightBlack,
                quote: AnsiColor::BrightBlack,
                rule: AnsiColor::BrightBlack,
                muted: AnsiColor::BrightBlack,
                accent: AnsiColor::Cyan,
            },
            ThemeName::Light => Self {
                monochrome: false,
                heading: AnsiColor::Black,
                link: AnsiColor::BrightBlack,
                code: AnsiColor::BrightBlack,
                quote: AnsiColor::BrightBlack,
                rule: AnsiColor::BrightBlack,
                muted: AnsiColor::BrightBlack,
                accent: AnsiColor::Blue,
            },
            ThemeName::Minimal => Self {
                monochrome: true,
                heading: AnsiColor::White,
                link: AnsiColor::BrightBlack,
                code: AnsiColor::BrightBlack,
                quote: AnsiColor::BrightBlack,
                rule: AnsiColor::BrightBlack,
                muted: AnsiColor::BrightBlack,
                accent: AnsiColor::BrightBlack,
            },
        }
    }
}
