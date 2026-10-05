# HTTP downloads

The `Http` service gives a product outbound HTTPS without owning sockets,
TLS or threads. The product chooses URLs, reads what comes back and decides
what to install; the Engine runs the transfers. Nothing starts until the
product makes its first request, so an offline or single-player product needs
no network, account or configuration.

Mechanism: [`svc-http`](../rust/crates/svc-http) (ureq 3 over rustls, trusting
the platform's certificate store) behind the generated `Http` family in
[`csharp-engine-services`](../rust/crates/csharp-engine-services/src/http.rs).

## Transfers

- `Http.Get(HttpGetRequest)` fetches a body into memory. Empty header fields
  send no header; the default User-Agent is `rusty-engine/<version>`.
  `BearerToken` becomes `Authorization: Bearer …` and is sent only to the
  URL's own host, never to a redirect target on another host (GitHub release
  assets redirect to a CDN). Any HTTP status completes the transfer, so a
  `304 Not Modified` answer to `IfNoneMatch` is an ordinary completion.
- `Http.Download(HttpDownloadRequest)` writes the body to `FileName` in an
  `HttpLibrary`. The body goes to a hidden `.<name>.<id>.partial` file and is
  renamed into place, replacing a file of that name, only once all of it
  arrived. A status other than 2xx, a short body, a cancel or a stall removes
  the partial file, so the library never holds a file that looks complete but
  is not. A download that fails is started again from the beginning; there is
  no range resume. Starting a download removes partial files of the same name
  that a stopped process left behind.
- Redirects are followed, up to ten.

Each transfer returns a disposable `HttpTransfer`. Disposing it cancels the
transfer if it is still running.

## Observing a transfer

Transfers run on Engine threads and never call into the product. Each product
call begins with a fresh snapshot of every transfer, and every read during
that call sees the same snapshot:

- `Http.Read(transfer)`: `State` (`Running`, `Completed`, `Failed`,
  `Cancelled`), `Failure`, the final response's `Status`, `ReceivedBytes` and
  `ExpectedBytes` (zero when the server announced no length). Poll it from
  `Update` to show progress.
- `Http.ReadHeaders(transfer)`: the final response's headers with lower-case
  names, for `etag`, `x-ratelimit-remaining`, `x-ratelimit-reset` and the like.
- `Http.ReadBody(transfer)`: a completed GET's body. A download keeps none.
- `Http.ReadDiagnostic(transfer)`: UTF-8 text explaining a failure.
- `Http.Cancel(transfer)`: the state is `Cancelled` at once and stays so.

`Failure` says what went wrong: `Connect` (name resolution, connection or
TLS), `Interrupted` (the connection broke, the body ended early, or the
transfer stalled), `Status` (a download answered other than 2xx),
`BodyTooLarge`, `Storage` (writing or moving the file) or `Protocol` (broken
HTTP or too many redirects). A bad URL or library file name is refused at the
call with `EngineCallException` (`CSHARP_HTTP`).

## Libraries

`Http.OpenLibrary(path)` names a directory, absolute or relative to the
process directory, and creates it if needed. `ReadLibraryFiles` lists its
complete files (never partial downloads) and `RemoveLibraryFile` removes one
by name. A library file name is one path component. Open a downloaded
container with `Content.OpenContainer` and check its identity with
`Content.ReadBundleIdentity`; the Engine adds no second hash.

## Limits

Each limit answers one failure:

| Limit | Value | Why | Cost |
| --- | --- | --- | --- |
| GET body | 32 MiB | A GET of a large asset would grow memory without bound. | Larger bodies must be downloaded to a library. |
| Stall | 60 s without data | A connection that silently drops would otherwise block until the operating system gives up on the socket, often hours. | A transfer that legitimately receives nothing for a minute fails and is restarted. |
| Connect | 30 s | An unreachable host. | |
| Response headers | 60 s after sending | A server that accepts and never answers. | |

Each running transfer uses one thread and its own connections. A blocked
read wakes every 250 ms, so a cancelled or stalled transfer ends its thread
and removes its partial file within that time, whether or not the product
is looking.

## Example

[`fixtures/csharp-http-downloads`](../fixtures/csharp-http-downloads) reads a
GitHub release from the REST API (revalidated with its ETag), downloads the
release's container asset through the CDN redirect, cancels and restarts it,
reports a refused connection, and opens the result with
`Content.OpenContainer`.
