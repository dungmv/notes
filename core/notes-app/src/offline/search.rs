use serde::Serialize;

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct TextMatch {
    pub start: u32,
    pub end: u32,
}

pub(super) fn fold(text: &str) -> String {
    folded_chars(text).map(|(ch, _, _)| ch).collect()
}

pub fn find_in_text(text: &str, input: &str) -> Vec<TextMatch> {
    let needle = fold(input.trim());
    if needle.is_empty() {
        return Vec::new();
    }
    let mut folded = String::new();
    let mapped: Vec<_> = folded_chars(text)
        .map(|(ch, start, end)| {
            let position = folded.len();
            folded.push(ch);
            (position, start, end)
        })
        .collect();
    folded
        .match_indices(&needle)
        .map(|(start, value)| {
            let end = start + value.len();
            let first = mapped.partition_point(|(position, _, _)| *position < start);
            let last = mapped.partition_point(|(position, _, _)| *position < end) - 1;
            TextMatch {
                start: mapped[first].1,
                end: mapped[last].2,
            }
        })
        .collect()
}

fn folded_chars(text: &str) -> impl Iterator<Item = (char, u32, u32)> + '_ {
    let mut chars = text.chars().peekable();
    let mut offset = 0;
    std::iter::from_fn(move || {
        while let Some(ch) = chars.next() {
            let start = offset;
            offset += ch.len_utf16() as u32;
            if vietnamese_mark(ch) {
                continue;
            }
            while chars.next_if(|ch| vietnamese_mark(*ch)).is_some() {
                offset += 1;
            }
            let end = offset;
            return Some(
                ch.to_lowercase()
                    .map(move |ch| (fold_vietnamese(ch), start, end)),
            );
        }
        None
    })
    .flatten()
}

fn vietnamese_mark(ch: char) -> bool {
    matches!(
        ch,
        '\u{300}'
            | '\u{301}'
            | '\u{302}'
            | '\u{303}'
            | '\u{306}'
            | '\u{309}'
            | '\u{31b}'
            | '\u{323}'
    )
}

fn fold_vietnamese(ch: char) -> char {
    match ch {
        'à' | 'á' | 'ả' | 'ã' | 'ạ' | 'ă' | 'ằ' | 'ắ' | 'ẳ' | 'ẵ' | 'ặ' | 'â' | 'ầ' | 'ấ' | 'ẩ'
        | 'ẫ' | 'ậ' => 'a',
        'è' | 'é' | 'ẻ' | 'ẽ' | 'ẹ' | 'ê' | 'ề' | 'ế' | 'ể' | 'ễ' | 'ệ' => 'e',
        'ì' | 'í' | 'ỉ' | 'ĩ' | 'ị' => 'i',
        'ò' | 'ó' | 'ỏ' | 'õ' | 'ọ' | 'ô' | 'ồ' | 'ố' | 'ổ' | 'ỗ' | 'ộ' | 'ơ' | 'ờ' | 'ớ' | 'ở'
        | 'ỡ' | 'ợ' => 'o',
        'ù' | 'ú' | 'ủ' | 'ũ' | 'ụ' | 'ư' | 'ừ' | 'ứ' | 'ử' | 'ữ' | 'ự' => 'u',
        'ỳ' | 'ý' | 'ỷ' | 'ỹ' | 'ỵ' => 'y',
        'đ' => 'd',
        _ => ch,
    }
}
