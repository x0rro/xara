use crate::crc::crc16_genibus;

pub fn call_hash(call: &[u8]) -> u32 {
    if call.is_empty() {
        return 0;
    }
    let c1 = u32::from(crc16_genibus(call));
    let c2 = u32::from(crc16_genibus(&call[call.len().saturating_sub(3)..]));
    ((c2 >> 8) << 16) | c1
}

/// SSID suffix as VARA packs it after the hyphen code: 1-15 numeric, T/R/X/L as 16-19
/// (so a numeric 16-19 is the same thing on air).
fn ssid_code(s: &[u8]) -> Option<u8> {
    match s {
        b"T" => Some(16),
        b"R" => Some(17),
        b"X" => Some(18),
        b"L" => Some(19),
        _ => std::str::from_utf8(s).ok()?.parse::<u8>().ok().filter(|n| (1..=19).contains(n)),
    }
}

fn ssid_text(code: u8) -> String {
    match code {
        16 => "T".into(),
        17 => "R".into(),
        18 => "X".into(),
        19 => "L".into(),
        n => n.to_string(),
    }
}

/// Eight 6-bit codes: the base call in the first seven (A-Z = 1-26, 0-9 = 27-36, anything else
/// and padding = 0) and the SSID code in the eighth (0 = none). There is no hyphen on air.
pub fn pack_call(call: &[u8]) -> [u8; 6] {
    let (base, ssid) = match call.iter().position(|&c| c == b'-') {
        Some(i) => (&call[..i], ssid_code(&call[i + 1..])),
        None => (call, None),
    };
    let mut codes = [0u8; 8];
    for (slot, &c) in codes[..7].iter_mut().zip(base) {
        *slot = match c {
            b'A'..=b'Z' => c - b'A' + 1,
            b'0'..=b'9' => c - b'0' + 27,
            _ => 0,
        };
    }
    codes[7] = ssid.unwrap_or(0);
    let bits = codes.iter().fold(0u64, |v, &c| (v << 6) | u64::from(c));
    bits.to_be_bytes()[2..].try_into().unwrap()
}

pub fn is_us_call(call: &str) -> bool {
    let base = call.split('-').next().unwrap_or("").as_bytes();
    let n = base.len();
    let digits: Vec<usize> = (0..n).filter(|&i| base[i].is_ascii_digit()).collect();
    if !(4..=6).contains(&n) || !b"NKWA".contains(&base[0]) || digits.len() != 1 || !(1..=2).contains(&digits[0]) {
        return false;
    }
    if !base.iter().all(u8::is_ascii_alphanumeric) {
        return false;
    }
    if base[0] == b'A' && (b'M'..=b'Z').contains(&base[1]) {
        return false;
    }
    let (pre, suf) = (digits[0], n - digits[0] - 1);
    match (pre, suf) {
        (2, 3) => matches!(base[0], b'K' | b'W'),
        (2, 2) if base[0] == b'A' => (b'A'..=b'L').contains(&base[1]),
        _ => true,
    }
}

pub fn unpack_call(b: &[u8]) -> Option<String> {
    let b = b.first_chunk::<6>()?;
    let v = b.iter().fold(0u64, |v, &x| (v << 8) | u64::from(x));
    let codes: Vec<u8> = (0..8).rev().map(|i| ((v >> (6 * i)) & 63) as u8).collect();
    let mut s = String::new();
    for &c in &codes[..7] {
        match c {
            1..=26 => s.push((b'A' + c - 1) as char),
            27..=36 => s.push((b'0' + c - 27) as char),
            _ => break,
        }
    }
    if let n @ 1..=19 = codes[7] {
        s.push('-');
        s.push_str(&ssid_text(n));
    }
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn from_codes(c: [u8; 8]) -> [u8; 6] {
        c.iter().fold(0u64, |v, &x| (v << 6) | u64::from(x)).to_be_bytes()[2..].try_into().unwrap()
    }

    #[test]
    fn spec_examples() {
        assert_eq!(call_hash(b"N0CALL"), 0xD80986);
        assert_eq!(call_hash(b""), 0);
        assert_eq!(pack_call(b"N0CALL"), [0x39, 0xb0, 0xc1, 0x30, 0xc0, 0x00]);
        assert_eq!(unpack_call(&pack_call(b"N0CALL")).as_deref(), Some("N0CALL"));
    }

    #[test]
    fn ssid_as_vara_4_9_0_sends_it() {
        // header codes captured from VARA HF 4.9.0 calling with these MYCALLs
        let seen = [
            ("OE6HKE-2", [15, 5, 33, 8, 11, 5, 0, 2]),
            ("OE6HKE-10", [15, 5, 33, 8, 11, 5, 0, 10]),
            ("OE6HKE-15", [15, 5, 33, 8, 11, 5, 0, 15]),
            ("OE6HKE-T", [15, 5, 33, 8, 11, 5, 0, 16]),
            ("OE6HKE-R", [15, 5, 33, 8, 11, 5, 0, 17]),
            ("OE6HKE-X", [15, 5, 33, 8, 11, 5, 0, 18]),
            ("OE6HKE-L", [15, 5, 33, 8, 11, 5, 0, 19]),
            ("W1AW-5", [23, 28, 1, 23, 0, 0, 0, 5]),
            ("OE6HK-3", [15, 5, 33, 8, 11, 0, 0, 3]),
            ("OE6HKEA-1", [15, 5, 33, 8, 11, 5, 1, 1]),
            ("OE6HKEA", [15, 5, 33, 8, 11, 5, 1, 0]),
        ];
        for (call, codes) in seen {
            assert_eq!(pack_call(call.as_bytes()), from_codes(codes), "{call}");
            assert_eq!(unpack_call(&from_codes(codes)).as_deref(), Some(call), "{call}");
        }
        // a numeric 16-19 is the same thing on air as T/R/X/L
        assert_eq!(pack_call(b"OE6HKE-19"), pack_call(b"OE6HKE-L"));
        // bad suffixes pack as the bare call
        assert_eq!(pack_call(b"N0CALL-0"), pack_call(b"N0CALL"));
        assert_eq!(pack_call(b"N0CALL-Q"), pack_call(b"N0CALL"));
    }
}
