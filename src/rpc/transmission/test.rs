use super::*;

#[test]
fn test_url() {
    let transmission = Transmission::new("127.0.0.1:9091", None).unwrap();
    assert_eq!(transmission.url(), "http://127.0.0.1:9091/transmission/rpc");
}
