use crate::ty::Ty;

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedIntLiteral {
    pub value: i64,
    pub ty: Ty,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedFloatLiteral {
    pub value: f64,
    pub ty: Ty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericLiteralErrorKind {
    Invalid,
    OutOfRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumericLiteralError {
    pub kind: NumericLiteralErrorKind,
    pub message: String,
}

impl NumericLiteralError {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            kind: NumericLiteralErrorKind::Invalid,
            message: message.into(),
        }
    }

    fn out_of_range(message: impl Into<String>) -> Self {
        Self {
            kind: NumericLiteralErrorKind::OutOfRange,
            message: message.into(),
        }
    }
}

pub fn parse_int_literal(raw: &str) -> Result<ParsedIntLiteral, NumericLiteralError> {
    let compact = raw.replace('_', "");
    let (body, radix) = strip_int_radix(&compact)
        .ok_or_else(|| NumericLiteralError::invalid(format!("invalid integer literal `{raw}`")))?;
    let (digits, suffix) = split_int_digits_suffix(body, radix)
        .ok_or_else(|| NumericLiteralError::invalid(format!("invalid integer literal `{raw}`")))?;
    let ty = int_suffix_ty(suffix, raw)?;

    if digits.is_empty() {
        return Err(NumericLiteralError::invalid(format!(
            "invalid integer literal `{raw}`"
        )));
    }

    let parsed = u128::from_str_radix(digits, radix)
        .map_err(|_| NumericLiteralError::invalid(format!("invalid integer literal `{raw}`")))?;
    let max = int_literal_max(&ty);
    if parsed > max {
        return Err(NumericLiteralError::out_of_range(format!(
            "integer literal `{raw}` is out of range for `{}`",
            ty.display()
        )));
    }

    Ok(ParsedIntLiteral {
        value: parsed as i64,
        ty,
    })
}

pub fn parse_float_literal(raw: &str) -> Result<ParsedFloatLiteral, NumericLiteralError> {
    let compact = raw.replace('_', "");
    let (number, suffix) = split_float_number_suffix(&compact, raw)?;
    let ty = float_suffix_ty(suffix, raw)?;
    let value: f64 = number
        .parse()
        .map_err(|_| NumericLiteralError::invalid(format!("invalid float literal `{raw}`")))?;

    if !value.is_finite() {
        return Err(NumericLiteralError::out_of_range(format!(
            "float literal `{raw}` is out of range for `{}`",
            ty.display()
        )));
    }
    if matches!(ty, Ty::Float32) && !((value as f32).is_finite()) {
        return Err(NumericLiteralError::out_of_range(format!(
            "float literal `{raw}` is out of range for `float32`"
        )));
    }

    Ok(ParsedFloatLiteral { value, ty })
}

fn split_int_digits_suffix(input: &str, radix: u32) -> Option<(&str, Option<&str>)> {
    let end = input
        .char_indices()
        .take_while(|(_, ch)| is_digit_for_radix(*ch, radix))
        .map(|(idx, ch)| idx + ch.len_utf8())
        .last()
        .unwrap_or(0);
    let (digits, rest) = input.split_at(end);
    if digits.is_empty() {
        return None;
    }
    if rest.is_empty() {
        return Some((digits, None));
    }
    if rest.chars().next().is_some_and(is_suffix_start) {
        Some((digits, Some(rest)))
    } else {
        None
    }
}

fn int_suffix_ty(suffix: Option<&str>, raw: &str) -> Result<Ty, NumericLiteralError> {
    match suffix {
        Some("i8") => Ok(Ty::Int8),
        Some("i16") => Ok(Ty::Int16),
        Some("i32") => Ok(Ty::Int32),
        Some("i64") => Ok(Ty::Int64),
        Some("u8") => Ok(Ty::U8),
        Some("u16") => Ok(Ty::U16),
        Some("u32") => Ok(Ty::U32),
        Some("u64") => Ok(Ty::U64),
        Some(other) => Err(NumericLiteralError::invalid(format!(
            "invalid integer literal suffix `{other}` in `{raw}`"
        ))),
        None => Ok(Ty::Int),
    }
}

fn strip_int_radix(input: &str) -> Option<(&str, u32)> {
    if let Some(hex) = input
        .strip_prefix("0x")
        .or_else(|| input.strip_prefix("0X"))
    {
        Some((hex, 16))
    } else if let Some(bin) = input
        .strip_prefix("0b")
        .or_else(|| input.strip_prefix("0B"))
    {
        Some((bin, 2))
    } else if let Some(oct) = input
        .strip_prefix("0o")
        .or_else(|| input.strip_prefix("0O"))
    {
        Some((oct, 8))
    } else {
        Some((input, 10))
    }
}

fn is_digit_for_radix(ch: char, radix: u32) -> bool {
    match radix {
        2 => matches!(ch, '0' | '1'),
        8 => matches!(ch, '0'..='7'),
        10 => ch.is_ascii_digit(),
        16 => ch.is_ascii_hexdigit(),
        _ => false,
    }
}

fn int_literal_max(ty: &Ty) -> u128 {
    match ty {
        Ty::Int8 => i8::MAX as u128,
        Ty::Int16 => i16::MAX as u128,
        Ty::Int32 => i32::MAX as u128,
        Ty::Int | Ty::Int64 => i64::MAX as u128,
        Ty::U8 => u8::MAX as u128,
        Ty::U16 => u16::MAX as u128,
        Ty::U32 => u32::MAX as u128,
        // HIR stores the raw bits in i64 today. Values above i64::MAX are
        // carried as the equivalent two's-complement bit pattern.
        Ty::U64 => u64::MAX as u128,
        _ => i64::MAX as u128,
    }
}

fn split_float_number_suffix<'a>(
    input: &'a str,
    raw: &str,
) -> Result<(&'a str, Option<&'a str>), NumericLiteralError> {
    let bytes = input.as_bytes();
    let mut idx = consume_ascii_digits(bytes, 0);
    if idx == 0 || bytes.get(idx) != Some(&b'.') {
        return Err(NumericLiteralError::invalid(format!(
            "invalid float literal `{raw}`"
        )));
    }

    idx += 1;
    let frac_start = idx;
    idx = consume_ascii_digits(bytes, idx);
    if idx == frac_start {
        return Err(NumericLiteralError::invalid(format!(
            "invalid float literal `{raw}`"
        )));
    }

    if matches!(bytes.get(idx), Some(b'e' | b'E')) {
        idx += 1;
        if matches!(bytes.get(idx), Some(b'+' | b'-')) {
            idx += 1;
        }
        let exp_start = idx;
        idx = consume_ascii_digits(bytes, idx);
        if idx == exp_start {
            return Err(NumericLiteralError::invalid(format!(
                "invalid float literal `{raw}`"
            )));
        }
    }

    let (number, rest) = input.split_at(idx);
    if rest.is_empty() {
        return Ok((number, None));
    }
    if rest.chars().next().is_some_and(is_suffix_start) {
        Ok((number, Some(rest)))
    } else {
        Err(NumericLiteralError::invalid(format!(
            "invalid float literal `{raw}`"
        )))
    }
}

fn consume_ascii_digits(bytes: &[u8], mut idx: usize) -> usize {
    while bytes.get(idx).is_some_and(|byte| byte.is_ascii_digit()) {
        idx += 1;
    }
    idx
}

fn is_suffix_start(ch: char) -> bool {
    ch == '_' || ch.is_alphabetic()
}

fn float_suffix_ty(suffix: Option<&str>, raw: &str) -> Result<Ty, NumericLiteralError> {
    match suffix {
        Some("f32") => Ok(Ty::Float32),
        Some("f64") => Ok(Ty::Float64),
        Some(other) => Err(NumericLiteralError::invalid(format!(
            "invalid float literal suffix `{other}` in `{raw}`"
        ))),
        None => Ok(Ty::Float),
    }
}
