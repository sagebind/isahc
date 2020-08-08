use test_case::test_case;
use testserver::endpoint;

#[test_case(200)]
#[test_case(202)]
#[test_case(204)]
#[test_case(302)]
#[test_case(308)]
#[test_case(400)]
#[test_case(403)]
#[test_case(404)]
#[test_case(418)]
#[test_case(429)]
#[test_case(451)]
#[test_case(500)]
#[test_case(503)]
fn returns_correct_response_code(status: u16) {
    let endpoint = endpoint! {
        status_code: status,
    };

    let response = isahc::get(endpoint.url()).unwrap();

    assert_eq!(response.status(), status);
    assert_eq!(endpoint.requests().len(), 1);
}
