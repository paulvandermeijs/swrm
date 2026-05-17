use alacritty_terminal::vte::ansi::{NamedColor, Rgb};

/// Standard 16 ANSI colors + 240 xterm 256-color palette entries.
pub const ANSI_256: [Rgb; 256] = build_palette();

pub const DEFAULT_FG: Rgb = Rgb {
    r: 0xee,
    g: 0xee,
    b: 0xee,
};
pub const DEFAULT_BG: Rgb = Rgb {
    r: 0x11,
    g: 0x11,
    b: 0x11,
};

pub fn resolve_named(name: NamedColor) -> Rgb {
    match name {
        NamedColor::Foreground => DEFAULT_FG,
        NamedColor::Background => DEFAULT_BG,
        NamedColor::Cursor => DEFAULT_FG,
        NamedColor::Black => ANSI_256[0],
        NamedColor::Red => ANSI_256[1],
        NamedColor::Green => ANSI_256[2],
        NamedColor::Yellow => ANSI_256[3],
        NamedColor::Blue => ANSI_256[4],
        NamedColor::Magenta => ANSI_256[5],
        NamedColor::Cyan => ANSI_256[6],
        NamedColor::White => ANSI_256[7],
        NamedColor::BrightBlack => ANSI_256[8],
        NamedColor::BrightRed => ANSI_256[9],
        NamedColor::BrightGreen => ANSI_256[10],
        NamedColor::BrightYellow => ANSI_256[11],
        NamedColor::BrightBlue => ANSI_256[12],
        NamedColor::BrightMagenta => ANSI_256[13],
        NamedColor::BrightCyan => ANSI_256[14],
        NamedColor::BrightWhite => ANSI_256[15],
        NamedColor::BrightForeground => DEFAULT_FG,
        NamedColor::DimForeground => DEFAULT_FG,
        NamedColor::DimBlack => ANSI_256[0],
        NamedColor::DimRed => ANSI_256[1],
        NamedColor::DimGreen => ANSI_256[2],
        NamedColor::DimYellow => ANSI_256[3],
        NamedColor::DimBlue => ANSI_256[4],
        NamedColor::DimMagenta => ANSI_256[5],
        NamedColor::DimCyan => ANSI_256[6],
        NamedColor::DimWhite => ANSI_256[7],
    }
}

#[inline]
pub fn rgb_to_u32(c: Rgb) -> u32 {
    ((c.r as u32) << 16) | ((c.g as u32) << 8) | (c.b as u32)
}

const fn build_palette() -> [Rgb; 256] {
    let mut p = [Rgb { r: 0, g: 0, b: 0 }; 256];

    let base = [
        (0x00, 0x00, 0x00),
        (0xcd, 0x00, 0x00),
        (0x00, 0xcd, 0x00),
        (0xcd, 0xcd, 0x00),
        (0x00, 0x00, 0xee),
        (0xcd, 0x00, 0xcd),
        (0x00, 0xcd, 0xcd),
        (0xe5, 0xe5, 0xe5),
        (0x7f, 0x7f, 0x7f),
        (0xff, 0x00, 0x00),
        (0x00, 0xff, 0x00),
        (0xff, 0xff, 0x00),
        (0x5c, 0x5c, 0xff),
        (0xff, 0x00, 0xff),
        (0x00, 0xff, 0xff),
        (0xff, 0xff, 0xff),
    ];
    let mut i = 0;
    while i < 16 {
        p[i] = Rgb {
            r: base[i].0,
            g: base[i].1,
            b: base[i].2,
        };
        i += 1;
    }

    let levels = [0u8, 0x5f, 0x87, 0xaf, 0xd7, 0xff];
    let mut r = 0;
    while r < 6 {
        let mut g = 0;
        while g < 6 {
            let mut b = 0;
            while b < 6 {
                let idx = 16 + 36 * r + 6 * g + b;
                p[idx] = Rgb {
                    r: levels[r],
                    g: levels[g],
                    b: levels[b],
                };
                b += 1;
            }
            g += 1;
        }
        r += 1;
    }

    let mut k = 0;
    while k < 24 {
        let v = 8 + 10 * k as u8;
        p[232 + k] = Rgb { r: v, g: v, b: v };
        k += 1;
    }
    p
}
