
use super::actix_web;

#[test]
fn it_works() {
	assert_eq!(2 + 2, 4);
}

#[test]
fn compare_headers() {
	let h1 = actix_web::http::header::HeaderName::from_bytes(b"X-Forwarded-For").unwrap();
	let h2 = actix_web::http::header::HeaderName::from_lowercase(b"x-forwarded-for").unwrap();
	assert_eq!(h1, h2);
}