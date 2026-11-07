use horizon_client_font::bidi::visual_str;

#[test]
fn orders_mixed_hebrew_text_for_display() {
    assert_eq!(visual_str("hello אבג"), "hello גבא");
}

#[test]
fn shapes_and_orders_joined_arabic_text() {
    assert_eq!(visual_str("سلام"), "ﻡﻼﺳ");
}
