use futures::{
    AsyncRead as _, AsyncReadExt as _, AsyncWriteExt as _, FutureExt as _, StreamExt as _,
    TryStreamExt as _,
};
use isahc::config::Configurable as _;
use isahc::{
    AsyncBody, Request, Response,
    config::Dialer,
    http::{HeaderMap, StatusCode, Uri},
    send_async,
};
use std::{
    collections::HashSet,
    env, io, net, panic, path,
    pin::Pin,
    process,
    sync::{
        Arc, LazyLock,
        atomic::{AtomicU16, Ordering},
    },
    task::{Context, Poll},
    time,
};

static SCENARIO_ID: AtomicU16 = AtomicU16::new(1);

static FILE_WRITE_LOCKS: LazyLock<async_std::sync::Mutex<HashSet<async_std::path::PathBuf>>> =
    LazyLock::new(|| async_std::sync::Mutex::new(HashSet::new()));

static LAST_CHUNK_FINDER: LazyLock<memchr::memmem::Finder> =
    LazyLock::new(|| memchr::memmem::Finder::new(b"0\r\n\r\n"));

/// This integration test is used to reproduce a segmentation fault. For the segmentation fault to
/// occur the test usually needs to be run repeatedly.
///
/// The test makes multiple HTTP requests concurrently to an HTTP server. The client-side code of
/// the test is straightforward enough, but the server-side code is bizarre. The server-side code
/// needs to do all kinds of network I/O, filesystem I/O and thread synchronisation operations to
/// reproduce the conditions in which the segmentation fault can occur.
///
/// Below is a detailed list of the steps that lead to the segmentation fault when the conditions
/// are right:
///
/// 1.  [`isahc::agent::AgentBuilder::spawn`] calls [`isahc::agent::AgentContext::run`].
/// 2.  `run` calls [`isahc::agent::AgentContext::poll`] in a loop.
/// 3.  `poll` has a `while` loop over socket update events. Certain kinds of socket update events
///     result in the code taking an if-branch in which [`isahc::agent::selector::Selector::register`]
///     is called.
/// 4.  `register` calls either [`isahc::agent::selector::poller_add`] or [`isahc::agent::selector::poller_modify`].
///      Both return a [`std::io::Result`] enum.
/// 5.  `register` does some mapping for the `Result` enum. If the `Result` is the [`Err`] variant,
///     the [`std::io::Error`], that it holds, is passed to [`isahc::agent::selector::is_bad_socket_error`].
/// 6.  In the conditions that this integration test attempts to reproduce, the [`std::io::Error::kind`]
///     method of the error passed to `is_bad_socket_error` returns the [`std::io::ErrorKind::PermissionDenied`]
///     variant. This variant causes `is_bad_socket_error` to return `false`.
/// 7.  `is_bad_socket_error` returning `false` starts the descent to a segmentation fault. When it
///     returns `false`, `register` returns the [`Err`] variant as-is (without mapping it to an
///     [`Ok`] variant).
/// 8.  `poll` calls the [`std::io::Result::unwrap`] method on the [`Err`] variant. This causes a panic.
/// 9.  The panic causes the call stack to be unwound. First `poll` is unwound.
/// 10. Next to be unwound is `run`. `run` has taken ownership of `self`, and when `run` is unwound
///     `self`/[`isahc::agent::AgentContext`] is dropped. As the `AgentContext` holds a [`curl::multi::Easy2Handle`]
///     and that in turn holds a [`curl::multi::DetachGuard`], [`curl::multi::DetachGuard::drop`]
///     is called.
/// 11. `drop` calls [`curl::multi::DetachGuard::detach`].
/// 12. `detach` calls [`curl_sys::curl_multi_remove_handle`].
/// 13. `curl_multi_remove_handle` is a C function and it calls more C functions. Execution returns
///     to Rust via the [`curl::multi::Multi::_socket_function::cb`] callback.
/// 14. `cb` defines a closure and passes it to [`curl::panic::catch`].
/// 15. `catch` invokes the closure.
/// 16. The closure executes unsafe code which creates a reference to another closure `f` and calls
///     `f` using the reference. A segmentation fault occurs. `f` is defined in [`isahc::agent::AgentContext::new`].
#[test]
fn segfault() {
    async_std::task::block_on(async {
        let config = Arc::new(Config::new());

        let future = panic::AssertUnwindSafe(async {
            static REQ_COUNT: usize = 20;

            set_up_test(Arc::clone(&config)).await;
            let (server_join_handle, server_addr) = start_server(Arc::clone(&config)).await;

            let mut req_futures = Vec::with_capacity(REQ_COUNT);
            for _ in 0..REQ_COUNT {
                let file_stream = FileStream::build_from_path("tests/sample-files/64-kib")
                    .await
                    .unwrap();
                req_futures.push(make_http_request(
                    server_addr,
                    "POST",
                    "/",
                    None,
                    AsyncBody::from_reader(
                        // Converting a `futures::Stream` into a `futures::AsyncRead` is necessary
                        // for the segmentation fault to occur.
                        file_stream.into_async_read(),
                    ),
                ));
            }

            for res in futures::future::join_all(req_futures).await {
                if res.status() != StatusCode::NO_CONTENT {
                    assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
                }
            }

            server_join_handle.cancel().await;
        });

        let result = future.catch_unwind().await;

        clean_up_test(&config).await;

        if let Err(error) = result {
            panic::resume_unwind(error);
        }
    });
}

#[derive(Debug)]
struct Config {
    temp_dir: path::PathBuf,
}

impl Config {
    fn new() -> Self {
        let id = Id::new();

        let mut temp_dir = env::temp_dir();
        temp_dir.push(&id.0);

        Self { temp_dir }
    }
}

#[derive(Debug)]
struct Id(String);

impl Id {
    fn new() -> Self {
        let timestamp = time::UNIX_EPOCH.elapsed().unwrap().as_nanos();
        let process_id = process::id();
        let scenario_id = SCENARIO_ID.fetch_add(1, Ordering::Relaxed);
        Self(format!("{timestamp}-{process_id}-{scenario_id}"))
    }
}

async fn set_up_test(config: Arc<Config>) {
    async_std::fs::create_dir(&config.temp_dir).await.unwrap();
}

async fn clean_up_test(config: &Config) {
    async_std::fs::remove_dir_all(&config.temp_dir)
        .await
        .unwrap();
}

pub struct FileStream {
    file_handle: async_std::fs::File,
}

impl FileStream {
    #[must_use]
    pub fn new(file_handle: async_std::fs::File) -> Self {
        Self { file_handle }
    }

    pub async fn build_from_path(path: impl AsRef<path::Path>) -> Result<Self, io::Error> {
        let file_handle = async_std::fs::File::open(path.as_ref()).await?;

        Ok(Self::new(file_handle))
    }
}

impl futures::Stream for FileStream {
    type Item = Result<Vec<u8>, io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut buf: [u8; 8000] = [0; 8000];
        if let Poll::Ready(result) = Pin::new(&mut self.file_handle).poll_read(cx, &mut buf) {
            match result {
                Ok(bytes_read) => {
                    // The stream has reached EOF.
                    if bytes_read == 0 {
                        return Poll::Ready(None);
                    }

                    let mut bytes = Vec::with_capacity(bytes_read);
                    bytes.extend_from_slice(&buf[..bytes_read]);
                    return Poll::Ready(Some(Ok(bytes)));
                }
                // Reading file failed.
                Err(err) => return Poll::Ready(Some(Err(err))),
            }
        }

        Poll::Pending
    }
}

async fn make_http_request(
    server_addr: net::SocketAddr,
    method: &str,
    path_and_query: &str,
    headers: Option<HeaderMap>,
    body: impl Into<AsyncBody>,
) -> Response<AsyncBody> {
    let url = Uri::builder()
        .scheme("http")
        .authority("localhost")
        .path_and_query(path_and_query)
        .build()
        .unwrap();

    let builder = Request::builder()
        .method(method)
        .uri(url)
        .dial(Dialer::ip_socket(server_addr));

    let mut req = builder.body(body).unwrap();

    if let Some(headers) = headers {
        *req.headers_mut() = headers;
    }

    send_async(req).await.unwrap()
}

async fn start_server(config: Arc<Config>) -> (async_std::task::JoinHandle<()>, net::SocketAddr) {
    let listener = async_std::net::TcpListener::bind((net::Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let addr = listener.local_addr().unwrap();

    let join_handle = async_std::task::spawn(async move {
        while let Some(result) = listener.incoming().next().await {
            let mut tcp_stream = result.unwrap();
            let config_arc_clone = Arc::clone(&config);
            async_std::task::spawn(async move {
                tcp_stream
                    .write_all(b"HTTP/1.1 100 \r\n\r\n")
                    .await
                    .unwrap();
                handle_connection(&mut tcp_stream, config_arc_clone).await;
                tcp_stream.shutdown(net::Shutdown::Write).unwrap();

                // Keep the TCP stream open for a second before closing it altogether (by dropping
                // `tcp_stream`). This delay allows the client to receive the `connection: close`
                // HTTP header and close the connection on their end before the connection is closed by
                // us. This is necessary when we want to return a response before reading the entirety
                // of the client's request. In that case, without this delay, the client might
                // needlessly consider the request as failed and might output an error such as `Failed
                // sending data to the peer`.
                async_std::task::sleep(time::Duration::from_secs(1)).await;
            });
        }
    });

    (join_handle, addr)
}

async fn handle_connection(
    stream: &mut (impl futures::AsyncRead + futures::AsyncWrite + Send + Unpin),
    config: Arc<Config>,
) {
    let mut buf: [u8; 8000] = [0; 8000];

    // Reading some bytes from `stream`, before opening a file handle, is necessary for the
    // segmentation fault to occur.
    let _ = stream.read(&mut buf).await.unwrap();
    let _ = stream.read(&mut buf).await.unwrap();
    let _ = stream.read(&mut buf).await.unwrap();
    let _ = stream.read(&mut buf).await.unwrap();
    let _ = stream.read(&mut buf).await.unwrap();
    let _ = stream.read(&mut buf).await.unwrap();
    let _ = stream.read(&mut buf).await.unwrap();

    let file_in_interim_dir_file_path = async_std::path::PathBuf::from(config.temp_dir.join("foo"));

    // It is important to execute this regex. Otherwise the segmentation fault occurs only rarely.
    static FILE_PATH_REGEX: LazyLock<regex::bytes::Regex> =
        LazyLock::new(|| regex::bytes::Regex::new("^(aaaa|xxxx)?a(b|c)a([a-zA-Z]+)$").unwrap());
    let _ = FILE_PATH_REGEX.is_match(b"aaaaaaaaaaaaaa");

    // This synchronisation using a mutex is necessary for the segmentation fault to occur.
    let mut locks_guard = FILE_WRITE_LOCKS.lock().await;
    if locks_guard.contains(&file_in_interim_dir_file_path) {
        drop(locks_guard);
        stream
            .write_all(b"HTTP/1.1 429 \r\ncontent-length:0\r\nconnection:close\r\n\r\n")
            .await
            .unwrap();
        return;
    } else {
        locks_guard.insert(file_in_interim_dir_file_path.clone());
        drop(locks_guard);
    }

    if let Err(error) = async {
        async_std::fs::create_dir_all(&config.temp_dir).await?;

        // Holding file handle open over `stream.read(...)` is necessary for the segmentation fault to occur.
        let file_handle = async_std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&file_in_interim_dir_file_path)
            .await?;

        loop {
            let bytes_read = stream.read(&mut buf).await?;
            if bytes_read == 0
                || (bytes_read >= 5
                    && LAST_CHUNK_FINDER
                        .find(&buf[(bytes_read - 5)..bytes_read])
                        .is_some())
            {
                break;
            }
        }

        drop(file_handle);
        async_std::fs::remove_file(&file_in_interim_dir_file_path).await?;

        Ok::<(), io::Error>(())
    }
    .await
    {
        let _ = async_std::fs::remove_file(&file_in_interim_dir_file_path).await;
        let mut locks_guard = FILE_WRITE_LOCKS.lock().await;
        locks_guard.remove(&file_in_interim_dir_file_path);
        drop(locks_guard);
        panic::panic_any(error);
    }

    let mut locks_guard = FILE_WRITE_LOCKS.lock().await;
    locks_guard.remove(&file_in_interim_dir_file_path);
    drop(locks_guard);

    stream
        .write_all(b"HTTP/1.1 204 \r\nconnection:close\r\n\r\n")
        .await
        .unwrap();
}
