use isahc::prelude::*;
use mockito::{mock, server_url};
use std::{io, thread, time::Duration};

speculate::speculate! {
    before {
        env_logger::try_init().ok();
    }

    test "metrics are disabled by default" {
        let m = mock("GET", "/").create();

        let response = isahc::get(server_url()).unwrap();

        assert!(response.metrics().is_none());

        m.assert();
    }

    test "enabling metrics causes metrics to be collected" {
        let m = mock("POST", "/")
            .with_body_from_fn(|body| {
                thread::sleep(Duration::from_millis(10));
                body.write_all(b"hello world")?;
                Ok(())
            })
            .create();

        let mut response = Request::post(server_url())
            .enable_metrics()
            .body("hello server")
            .unwrap()
            .send()
            .unwrap();

        let metrics = response.metrics().unwrap().clone();

        assert_eq!(metrics.upload_progress(), (12, 12));

        io::copy(response.body_mut(), &mut io::sink()).unwrap();

        assert_eq!(metrics.download_progress().0, 11);
        assert!(metrics.total_time() > Duration::default()); // FIXME: Why is this always zero?

        m.assert();
    }
}
