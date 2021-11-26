use testserver::mock;

#[test]
fn http09_not_allowed_by_default() {
    let m = mock! {
        #1 => writer |writer| {},
    };

    assert_eq!(m.requests_received(), 0);
}
