#!/usr/bin/env python3
"""Stress test: 30000 concurrent Redis connections via redis-py."""
import sys, time, threading, argparse

parser = argparse.ArgumentParser()
parser.add_argument("--host", default="127.0.0.1")
parser.add_argument("--port", type=int, default=6379)
parser.add_argument("--password", default="testpass")
parser.add_argument("--connections", type=int, default=30000)
parser.add_argument("--timeout", type=int, default=10)
args = parser.parse_args()

import redis

results = {"ok": 0, "fail": 0, "ping_ok": 0, "info_ok": 0, "cinfo_ok": 0, "cslots_ok": 0}
lock = threading.Lock()
barrier = threading.Barrier(threading.active_count() + 1)  # placeholder, recreated below
stop_barrier = None

def worker(batch_size):
    conns = []
    for _ in range(batch_size):
        try:
            r = redis.Redis(host=args.host, port=args.port, password=args.password,
                            socket_timeout=args.timeout, socket_connect_timeout=args.timeout,
                            decode_responses=True)
            conns.append(r)
        except Exception as e:
            with lock:
                results["fail"] += 1

    barrier.wait()

    for r in conns:
        try:
            r.ping()
            with lock:
                results["ping_ok"] += 1
            r.info()
            with lock:
                results["info_ok"] += 1
            r.execute_command("CLUSTER", "INFO")
            with lock:
                results["cinfo_ok"] += 1
            r.execute_command("CLUSTER", "SLOTS")
            with lock:
                results["cslots_ok"] += 1
            with lock:
                results["ok"] += 1
        except Exception:
            with lock:
                results["fail"] += 1

    stop_barrier.wait()
    for r in conns:
        try:
            r.connection_pool.disconnect()
        except Exception:
            pass

total = args.connections
batch = min(500, total)
thread_count = max(1, total // batch)
barrier = threading.Barrier(thread_count + 1)
stop_barrier = threading.Barrier(thread_count + 1)

print(f"Connecting {total} clients ({thread_count} threads x {batch} each)...")
threads = []
t0 = time.time()

for _ in range(thread_count):
    t = threading.Thread(target=worker, args=(batch,))
    t.start()
    threads.append(t)

barrier.wait()
t_conn = time.time()
print(f"All connected in {t_conn - t0:.1f}s. Running PING/INFO/CLUSTER INFO/CLUSTER SLOTS...")

stop_barrier.wait()
elapsed = time.time() - t0
rate = total / elapsed if elapsed else 0

print(f"total={total} ok={results['ok']} fail={results['fail']} "
      f"ping={results['ping_ok']} info={results['info_ok']} "
      f"cluster-info={results['cinfo_ok']} cluster-slots={results['cslots_ok']} "
      f"time={elapsed:.1f}s rate={rate:.0f}/s")

for t in threads:
    t.join()

if results["fail"] > 0:
    print(f"FAIL: {results['fail']} connections failed")
    sys.exit(1)
print("PASS")
