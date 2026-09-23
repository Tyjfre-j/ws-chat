use ratatui::style::Color;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeName {
    #[default]
    Catppuccin,
    Nord,
    TokyoNight,
}

impl ThemeName {
    pub fn next(self) -> Self {
        match self {
            ThemeName::Catppuccin => ThemeName::Nord,
            ThemeName::Nord => ThemeName::TokyoNight,
            ThemeName::TokyoNight => ThemeName::Catppuccin,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ThemeName::Catppuccin => "Catppuccin",
            ThemeName::Nord => "Nord",
            ThemeName::TokyoNight => "Tokyo Night",
        }
    }
}

#[derive(Clone, Copy)]
pub struct Theme {
    pub background: Color,
    pub connected: Color,
    pub disconnected: Color,
    pub connecting: Color,
    pub text: Color,
    pub dim: Color,
    pub rule: Color,
}

const CATPPUCCIN: Theme = Theme {
    background: Color::Rgb(30, 30, 46),
    connected: Color::Rgb(166, 227, 161),
    disconnected: Color::Rgb(243, 139, 168),
    connecting: Color::Rgb(249, 226, 175),
    text: Color::Rgb(205, 214, 244),
    dim: Color::Rgb(108, 112, 134),
    rule: Color::Rgb(69, 71, 90),
};

const NORD: Theme = Theme {
    background: Color::Rgb(46, 52, 64),
    connected: Color::Rgb(163, 190, 140),
    disconnected: Color::Rgb(191, 97, 106),
    connecting: Color::Rgb(235, 203, 139),
    text: Color::Rgb(216, 222, 233),
    dim: Color::Rgb(76, 86, 106),
    rule: Color::Rgb(59, 66, 82),
};

const TOKYO_NIGHT: Theme = Theme {
    background: Color::Rgb(26, 27, 38),
    connected: Color::Rgb(158, 206, 106),
    disconnected: Color::Rgb(247, 118, 142),
    connecting: Color::Rgb(224, 175, 104),
    text: Color::Rgb(192, 202, 245),
    dim: Color::Rgb(86, 95, 137),
    rule: Color::Rgb(59, 66, 97),
};

pub fn theme_for(name: ThemeName) -> Theme {
    match name {
        ThemeName::Catppuccin => CATPPUCCIN,
        ThemeName::Nord => NORD,
        ThemeName::TokyoNight => TOKYO_NIGHT,
    }
}
