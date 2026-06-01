#!/usr/bin/env python3
"""Minimal reproduction of EloqKV crash on repeated flushall+write operations.

Usage:
  # Inside resp-compat Docker container:
  python3 repro_flushall_crash.py --host 172.28.10.11 --port 6379 --password testpass

Crash: txservice::ScanDeltaSizeCcForHashPartition::Execute NULL pointer deref
        at cc_request.h:8917, triggered by repeated flushall + sadd + srem.
"""
import redis
import argparse
import sys
import time

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="172.28.10.11")
    parser.add_argument("--port", type=int, default=6379)
    parser.add_argument("--password", default="testpass")
    parser.add_argument("--max-loops", type=int, default=500)
    parser.add_argument("--cluster", action="store_true")
    args = parser.parse_args()

    if args.cluster:
        r = redis.RedisCluster(host=args.host, port=args.port,
                               password=args.password, decode_responses=True)
    else:
        r = redis.Redis(host=args.host, port=args.port,
                        password=args.password, decode_responses=True)
        r.response_callbacks = {}

    print(f"Connected to {args.host}:{args.port} (cluster={args.cluster})")
    print(f"Running up to {args.max_loops} loops of: flushall + sadd + srandmember + srem")

    start = time.time()
    for i in range(args.max_loops):
        try:
            r.flushall()
            r.sadd("repro_set", "a", "b", "c", "d", "e")
            r.srandmember("repro_set", 3)
            r.srem("repro_set", "a", "b")
            r.scard("repro_set")
            if i % 25 == 0:
                elapsed = time.time() - start
                print(f"  loop {i}: OK ({elapsed:.1f}s)", flush=True)
        except Exception as e:
            elapsed = time.time() - start
            print(f"\nCRASH at loop {i} ({elapsed:.1f}s): {e}")
            sys.exit(1)

    elapsed = time.time() - start
    print(f"\nCompleted {args.max_loops} loops without crash ({elapsed:.1f}s)")
    sys.exit(0)

if __name__ == "__main__":
    main()
