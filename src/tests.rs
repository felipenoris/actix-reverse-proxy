
use parse_and_add_client_ip;
use actix_web::http::header::HeaderName;

#[test]
fn compare_headers() {
    let h1 = HeaderName::from_bytes(b"X-Forwarded-For").unwrap();
    let h2 = HeaderName::from_lowercase(b"x-forwarded-for").unwrap();
    assert_eq!(h1, h2);
}

#[test]
fn test_add_client_ip() {
    let mut header_value = String::from("192.168.25.12");
    let client_address = "127.0.0.1:8000";
    parse_and_add_client_ip(&mut header_value, client_address);
    assert_eq!(&header_value, "192.168.25.12, 127.0.0.1");
}
