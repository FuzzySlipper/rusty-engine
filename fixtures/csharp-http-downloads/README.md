# HTTP download proof

Build this ordinary product with an explicit `RustyEngineFixtureSdkVersion`
and `RestoreAdditionalProjectSources` pointing at the matching SDK feed, and
run it through that pair's `rusty dev --live-debug`. It needs internet access
to `api.github.com` and GitHub's release CDN.

The module is `fixture-module-1.0.0.rpak` (about 150 MB of random data packed
with `rusty pack-content`) on the `fixture-http-downloads-v1` prerelease of
this repository. The library is `rusty-http-downloads-fixture/modules` under
the system temporary directory.

- `http.index` reads the release from the GitHub REST API. After the first
  answer it sends the ETag as `If-None-Match`, and GitHub answers 304.
- `http.inspect` reports every transfer, the ETag and rate-limit headers, and
  the asset URL taken from the release JSON.
- `http.download` downloads the asset into the library; `Update` logs
  `HTTP_FIXTURE_PROGRESS` every 16 MiB.
- `http.cancel` cancels it; `http.library` then lists no file. Run
  `http.download` again to start over.
- `http.fail` requests a refused port and reports a `Connect` failure.
- `http.library` lists the library, `http.open` opens the container with
  `Content.OpenContainer` and prints its file count and bundle identity, and
  `http.remove` removes it.
