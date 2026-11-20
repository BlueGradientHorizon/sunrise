use super::*;

#[derive(Debug, PartialEq, sunrise_mod_api_macros::Wire)]
struct Tiny {
    flag: bool,
    code: u8,
    more: Option<Box<u8>>,
}

#[derive(sunrise_mod_api_macros::Wire)]
struct Even(Option<Box<Odd>>);
#[derive(sunrise_mod_api_macros::Wire)]
struct Odd(Option<Box<Even>>);

#[test]
fn unchanged_forms_keep_the_previous_bytes_and_schema_including_mutual_recursion() {
    let value = Tiny {
        flag: true,
        code: 0xff,
        more: Some(Box::new(0xa3)),
    };
    let bytes = [1, 0xff, 1, 0xa3];
    assert_eq!(
        encode_payload(&value, PAYLOAD_DECODE_BUDGET).unwrap(),
        bytes
    );
    assert_eq!(decode_payload(&bytes), Ok(value));
    // Recorded against the former PayloadCodec layout, not calculated by Wire.
    assert_eq!(payload_schema::<Tiny>(), 0x1f02_846f_d6f3_c677);
    assert_eq!(payload_schema::<Even>(), 0x2ece_68d4_9675_316d);
    assert_eq!(payload_schema::<Odd>(), 0x2ece_68d4_9675_316d);
    let even = Even(Some(Box::new(Odd(None))));
    assert_eq!(encode_payload(&even, 2), Ok(vec![1, 0]));
    assert!(matches!(decode_payload::<Even>(&[1, 0]), Ok(Even(Some(odd))) if odd.0.is_none()));
}

#[derive(Debug, PartialEq, sunrise_mod_api_macros::Wire)]
enum Message {
    Ping,
    Data { value: u8 },
}

#[test]
fn the_canonical_enum_encoding_has_a_new_negotiated_fingerprint() {
    assert_ne!(payload_schema::<Message>(), 0x4c4c_7f32_2697_a7c3);
    let body = [1, 0, 0, 0, 7]; // fixed little-endian tag, then its one-byte field
    assert_eq!(
        encode_payload(&Message::Data { value: 7 }, 5).unwrap(),
        body
    );
    assert_eq!(decode_payload(&body), Ok(Message::Data { value: 7 }));
    assert_eq!(decode_payload::<Message>(&[1, 7]), Err(WireError::Eof));
}

#[test]
fn both_codecs_of_a_typed_body_refuse_excess_allocation_before_reserving() {
    // One byte per None in the body, but a Vec entry large enough to cross the
    // allocation budget. Exactly 4096 entries meets the distinct collection cap.
    type Wide = Vec<Option<[u8; 2048]>>;
    let mut bytes = vec![0, 0x10, 0, 0];
    bytes.resize(4 + 4096, 0);
    assert_eq!(decode_payload::<Wide>(&bytes), Err(WireError::Budget));
    assert_eq!(
        encode_payload(&vec![None::<[u8; 2048]>; 4096], PAYLOAD_DECODE_BUDGET),
        Err(WireError::Budget)
    );
}

#[test]
fn typed_caps_do_not_cap_ordinary_wire() {
    let value = "a".repeat(4097);
    let mut writer = Writer::new();
    writer.put(&value);
    assert_eq!(Reader::decode::<String>(writer.bytes()), Ok(value.clone()));
    assert_eq!(
        decode_payload::<String>(writer.bytes()),
        Err(WireError::Length)
    );
    assert_eq!(
        encode_payload(&value, PAYLOAD_DECODE_BUDGET),
        Err(WireError::Length)
    );
    assert_eq!(
        encode_payload(&"hello".to_owned(), 8),
        Err(WireError::Length)
    );
}
