//! Optional translation of the game's text (port extra). Loaded from `lang-<code>.txt` next to the
//! saves, never shipped: lines `T<idx>\t<text>` (dialogue), `I<idx>` (items), `L<idx>` (hotspot
//! labels), UTF-8, `\r` for a line break. Missing entries fall back to the original Czech.

use std::collections::HashMap;

#[derive(Default, Debug)]
pub struct Lang {
    pub texts: HashMap<usize, Vec<u8>>,
    pub items: HashMap<usize, Vec<u8>>,
    pub labels: HashMap<usize, Vec<u8>>,
}

pub fn file(code: &str) -> String {
    format!("lang-{code}.txt")
}

impl Lang {
    pub fn parse(text: &str) -> Self {
        let mut l = Lang::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('\t') else { continue };
            let (tag, idx) = key.split_at(1);
            let Ok(i) = idx.parse::<usize>() else { continue };
            let v = crate::reader::to_cp1250(&value.replace("\\r", "\r").replace("\\n", ""));
            match tag {
                "T" => l.texts.insert(i, v),
                "I" => l.items.insert(i, v),
                "L" => l.labels.insert(i, v),
                _ => None,
            };
        }
        l
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_tags() {
        let l = super::Lang::parse("T5\tHello\\rworld\nI2\tRope\nL7\tTree\nbad line\n");
        assert_eq!(l.texts[&5], b"Hello\rworld");
        assert_eq!(l.items[&2], b"Rope");
        assert_eq!(l.labels[&7], b"Tree");
    }
}
