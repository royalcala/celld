#!/usr/bin/env python3
"""SigV4 S3 probe client for the us-s3gw verification scripts (stdlib only).

Subcommands:
  race  24 concurrent `If-None-Match: *` PUTs against one key -> exactly 1 winner.
  bulk  single PUT and multipart measurements at the sizes celld stores.

The client signs every request for real (AWS SigV4, service `s3`), because
us-s3gw verifies signatures when started with --access-key/--secret-key.
"""

from __future__ import annotations

import argparse
import hashlib
import hmac
import http.client
import os
import sys
import threading
import time
import xml.etree.ElementTree as ET
from datetime import datetime, timezone
from urllib.parse import quote

MIB = 1024 * 1024


def _sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _hmac(key: bytes, msg: str) -> bytes:
    return hmac.new(key, msg.encode(), hashlib.sha256).digest()


class SigV4:
    def __init__(self, endpoint: str, access: str, secret: str, region: str = "us-east-1"):
        if endpoint.startswith("http://"):
            self.host = endpoint[len("http://"):]
            self.scheme = "http"
        elif endpoint.startswith("https://"):
            self.host = endpoint[len("https://"):]
            self.scheme = "https"
        else:
            self.host = endpoint
            self.scheme = "http"
        self.endpoint = endpoint.rstrip("/")
        self.access = access
        self.secret = secret
        self.region = region

    def signed_headers(
        self,
        method: str,
        path: str,
        query: list[tuple[str, str]],
        payload: bytes,
        extra: dict[str, str] | None = None,
    ) -> dict[str, str]:
        """Headers for the request, including a valid Authorization header."""
        amz_date = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
        date = amz_date[:8]
        payload_hash = _sha256_hex(payload)

        headers: dict[str, str] = {"host": self.host, "x-amz-content-sha256": payload_hash, "x-amz-date": amz_date}
        if extra:
            headers.update({k.lower(): v for k, v in extra.items()})

        # sign host + the two required x-amz-* headers (extra headers stay unsigned)
        signed = ["host", "x-amz-content-sha256", "x-amz-date"]
        canonical_headers = "".join(f"{k}:{headers[k].strip()}\n" for k in signed)
        signed_headers_str = ";".join(signed)

        canonical_query = "&".join(
            f"{quote(k, safe='-_.~')}={quote(v, safe='-_.~')}" for k, v in sorted(query)
        )
        canonical_request = "\n".join(
            [
                method,
                quote(path, safe="/-_.~"),
                canonical_query,
                canonical_headers,
                signed_headers_str,
                payload_hash,
            ]
        )

        scope = f"{date}/{self.region}/s3/aws4_request"
        string_to_sign = "\n".join(
            ["AWS4-HMAC-SHA256", amz_date, scope, _sha256_hex(canonical_request.encode())]
        )
        key = _hmac(_hmac(_hmac(_hmac(f"AWS4{self.secret}".encode(), date), self.region), "s3"), "aws4_request")
        signature = hmac.new(key, string_to_sign.encode(), hashlib.sha256).hexdigest()

        headers["authorization"] = (
            f"AWS4-HMAC-SHA256 Credential={self.access}/{scope}, "
            f"SignedHeaders={signed_headers_str}, Signature={signature}"
        )
        return headers

    def request(
        self,
        method: str,
        path: str,
        query: list[tuple[str, str]],
        payload: bytes = b"",
        extra: dict[str, str] | None = None,
    ) -> tuple[int, dict[str, str], bytes]:
        headers = self.signed_headers(method, path, query, payload, extra)
        conn = http.client.HTTPConnection(self.host, timeout=120)
        try:
            url = path
            if query:
                url += "?" + "&".join(f"{quote(k, safe='-_.~')}={quote(v, safe='-_.~')}" for k, v in sorted(query))
            conn.request(method, url, body=payload, headers=headers)
            resp = conn.getresponse()
            body = resp.read()
            return resp.status, {k.lower(): v for k, v in resp.getheaders()}, body
        finally:
            conn.close()


def _parse_size(text: str) -> int:
    text = text.strip().upper()
    factor = 1
    for suffix, f in (("GIB", MIB * 1024), ("MIB", MIB), ("KIB", 1024), ("G", MIB * 1024), ("M", MIB), ("K", 1024)):
        if text.endswith(suffix):
            factor = f
            text = text[: -len(suffix)]
            break
    return int(text) * factor


def _delete(s3: SigV4, bucket: str, key: str) -> None:
    s3.request("DELETE", f"/{bucket}/{key}", [])


def cmd_race(args: argparse.Namespace) -> int:
    s3 = SigV4(args.endpoint, args.access, args.secret, args.region)
    path = f"/{args.bucket}/{args.key}"
    payload = b"us-s3gw race probe body" * 4
    failures = 0

    for rnd in range(1, args.rounds + 1):
        statuses: list[int | None] = [None] * args.n
        barrier = threading.Barrier(args.n)

        def worker(i: int) -> None:
            barrier.wait()
            try:
                status, _, _ = s3.request("PUT", path, [], payload, extra={"If-None-Match": "*"})
                statuses[i] = status
            except Exception as exc:  # noqa: BLE001 - report, then fail the round
                statuses[i] = -1
                print(f"  worker {i}: {exc}", file=sys.stderr)

        threads = [threading.Thread(target=worker, args=(i,)) for i in range(args.n)]
        for t in threads:
            t.start()
        for t in threads:
            t.join()

        winners = sum(1 for s in statuses if s is not None and 200 <= s < 300)
        rejected = sum(1 for s in statuses if s == 412)
        errors = sum(1 for s in statuses if s is None or s < 0 or not (200 <= s < 300 or s == 412))
        print(f"round {rnd}/{args.rounds}: winners={winners} precondition_failed={rejected} errors={errors} statuses={sorted(statuses, key=str)}")

        if args.assert_one and (winners != 1 or errors != 0):
            failures += 1
        _delete(s3, args.bucket, args.key)

    if args.assert_one:
        if failures:
            print(f"FAIL: {failures}/{args.rounds} rounds did not have exactly 1 winner", file=sys.stderr)
            return 1
        print(f"PASS: exactly 1 winner in every round ({args.n} concurrent If-None-Match: * PUTs, lock on)")
        return 0

    print("report-only mode: no assertion")
    return 0


def cmd_bulk(args: argparse.Namespace) -> int:
    s3 = SigV4(args.endpoint, args.access, args.secret, args.region)
    bucket = args.bucket
    rows: list[tuple[str, int, float, str, str]] = []
    failed = False

    def record(op: str, size: int, secs: float, etag: str, verify: str) -> None:
        nonlocal failed
        mibps = size / MIB / secs if secs > 0 else float("inf")
        rows.append((op, size, secs, etag, verify))
        if verify != "ok":
            failed = True
        print(f"{op:<26} {size:>10} B  {secs:7.3f} s  {mibps:8.1f} MiB/s  etag={etag or '-'}  {verify}")

    def verify_object(key: str, data: bytes) -> str:
        status, _, body = s3.request("GET", f"/{bucket}/{key}", [])
        if status != 200 or _sha256_hex(body) != _sha256_hex(data):
            return f"FAIL full GET status={status} len={len(body)}"
        # celld's startup probe requires ranged reads, so check a range too.
        if len(data) > 2 * 1024 * 1024:
            start = len(data) // 2
            end = start + 1023
            status, hdrs, body = s3.request(
                "GET", f"/{bucket}/{key}", [], extra={"Range": f"bytes={start}-{end}"}
            )
            if status != 206 or body != data[start : end + 1]:
                return f"FAIL ranged GET status={status} len={len(body)}"
        return "ok"

    # 1. Single PUTs: the sizes below celld's 5 MiB multipart threshold
    #    (L0 WAL capture ~512 KiB; deploy bundle 3.4 MiB measured).
    for i, size in enumerate(args.single):
        key = f"{args.key}-single-{i}"
        data = os.urandom(size)
        t0 = time.perf_counter()
        status, hdrs, _ = s3.request("PUT", f"/{bucket}/{key}", [], data)
        secs = time.perf_counter() - t0
        verify = "ok" if status in (200, 201) else f"FAIL PUT status={status}"
        if verify == "ok":
            verify = verify_object(key, data)
        record(f"single PUT {size // 1024}K", size, secs, hdrs.get("etag", "").strip('"'), verify)
        _delete(s3, bucket, key)

    # 2. Multipart: what celld's LTX client does at >= 5 MiB (5 MiB parts).
    for i, size in enumerate(args.multipart):
        key = f"{args.key}-multipart-{i}"
        data = os.urandom(size)
        part_size = 5 * MIB
        chunks = [data[o : o + part_size] for o in range(0, size, part_size)]
        t0 = time.perf_counter()
        status, _, body = s3.request("POST", f"/{bucket}/{key}", [("uploads", "")])
        if status != 200:
            record(f"multipart {size // MIB}M", size, 0.0, "-", f"FAIL create status={status} {body[:200]!r}")
            continue
        upload_id = ET.fromstring(body).findtext(".//{*}UploadId") or ET.fromstring(body).findtext("UploadId")
        if not upload_id:
            record(f"multipart {size // MIB}M", size, 0.0, "-", f"FAIL no uploadId {body[:200]!r}")
            continue
        parts_ok = True
        part_etags: list[str] = []
        for n, chunk in enumerate(chunks, start=1):
            status, hdrs, body = s3.request(
                "PUT", f"/{bucket}/{key}", [("partNumber", str(n)), ("uploadId", upload_id)], chunk
            )
            if status != 200:
                record(f"multipart {size // MIB}M", size, 0.0, "-", f"FAIL part {n} status={status} {body[:200]!r}")
                parts_ok = False
                break
            part_etags.append(hdrs.get("etag", ""))
        if not parts_ok:
            s3.request("POST", f"/{bucket}/{key}", [("uploadId", upload_id)])
            continue
        xml_body = "<CompleteMultipartUpload>" + "".join(
            f"<Part><PartNumber>{n}</PartNumber><ETag>{etag}</ETag></Part>"
            for n, etag in enumerate(part_etags, start=1)
        ) + "</CompleteMultipartUpload>"
        status, _, body = s3.request("POST", f"/{bucket}/{key}", [("uploadId", upload_id)], xml_body.encode())
        secs = time.perf_counter() - t0
        if status != 200:
            record(f"multipart {size // MIB}M", size, secs, "-", f"FAIL complete status={status} {body[:200]!r}")
            continue
        etag = (ET.fromstring(body).findtext(".//{*}ETag") or ET.fromstring(body).findtext("ETag") or "").strip('"')
        verify = "ok"
        if not etag.endswith(f"-{len(chunks)}"):
            verify = f"FAIL etag suffix != -{len(chunks)}"
        else:
            verify = verify_object(key, data)
        record(f"multipart {size // MIB}M ({len(chunks)}x5M)", size, secs, etag, verify)
        _delete(s3, bucket, key)

    print()
    for op, size, secs, etag, verify in rows:
        print(f"  {op:<30} {size:>10} B  {secs:7.3f} s  {verify}")
    if failed:
        print("FAIL: at least one bulk operation did not verify", file=sys.stderr)
        return 1
    print("PASS: every single PUT and multipart upload verified (sha256, ranged GET, etag)")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="cmd", required=True)

    def common(p: argparse.ArgumentParser) -> None:
        p.add_argument("--endpoint", required=True)
        p.add_argument("--bucket", default="fleet")
        p.add_argument("--access", default=os.environ.get("S3GW_ACCESS_KEY", "t42-test-access"))
        p.add_argument("--secret", default=os.environ.get("S3GW_SECRET_KEY", "t42-test-secret"))
        p.add_argument("--region", default="us-east-1")

    p_race = sub.add_parser("race", help="24 concurrent conditional PUTs -> 1 winner")
    common(p_race)
    p_race.add_argument("--n", type=int, default=24)
    p_race.add_argument("--rounds", type=int, default=5)
    p_race.add_argument("--key", default="t42/race")
    p_race.add_argument("--report-only", action="store_true", help="do not assert (used with US_S3GW_NO_CAS_LOCK=1)")
    p_race.set_defaults(func=cmd_race)

    p_bulk = sub.add_parser("bulk", help="single PUT vs multipart measurements")
    common(p_bulk)
    p_bulk.add_argument("--single", default="512K,4M", help="comma-separated sizes below the 5 MiB threshold")
    p_bulk.add_argument("--multipart", default="12M,64M", help="comma-separated sizes for multipart uploads")
    p_bulk.add_argument("--key", default="t42/bulk")
    p_bulk.set_defaults(func=cmd_bulk)

    args = parser.parse_args()
    if args.cmd == "race":
        args.assert_one = not args.report_only
    if args.cmd == "bulk":
        args.single = [_parse_size(s) for s in args.single.split(",") if s.strip()]
        args.multipart = [_parse_size(s) for s in args.multipart.split(",") if s.strip()]
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
