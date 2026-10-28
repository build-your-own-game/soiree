//! One framed message consumes all its bytes. A valid prefix followed by
//! another message or unknown trailing data is not a valid frame.

use serde::Deserialize;

pub fn decode<'de, T: Deserialize<'de>>(bytes: &'de [u8]) -> Result<T, postcard::Error> {
    let (message, remaining) = postcard::take_from_bytes(bytes)?;
    if !remaining.is_empty() {
        return Err(postcard::Error::DeserializeBadEncoding);
    }
    Ok(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use serde::Serialize;

    #[derive(Debug, PartialEq, Deserialize, Serialize)]
    enum Message {
        Tap,
        Text(alloc::string::String),
    }

    #[test]
    fn framed_messages_reject_valid_prefixes_and_incomplete_payloads() {
        let tap = postcard::to_allocvec(&Message::Tap).unwrap();
        assert_eq!(decode::<Message>(&tap), Ok(Message::Tap));
        for suffix in [vec![0], vec![1], vec![255, 255]] {
            let mut extended = tap.clone();
            extended.extend(suffix);
            assert!(decode::<Message>(&extended).is_err());
        }
        let text = postcard::to_allocvec(&Message::Text("é🙂".into())).unwrap();
        for end in 0..text.len() {
            assert!(decode::<Message>(&text[..end]).is_err());
        }
        assert!(decode::<Message>(&[2]).is_err(), "unknown variant");
        assert!(
            decode::<Message>(&[1, 2, 255, 255]).is_err(),
            "invalid UTF-8"
        );
        assert!(
            decode::<Message>(&[1, 255, 255, 255, 255, 15]).is_err(),
            "huge length without contents"
        );
    }
}
