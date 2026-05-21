#!/usr/bin/env python3
"""Stress test: 30000 concurrent Redis connections via redis-py."""
import sys, time, threading, argparse

parser = argparse.ArgumentParser()
parser.add_argument("--host", default="127.0.0.1")
parser.add_argument("--port", type=int, default=6379)
parser.add_argument("--password", default="testpass")
parser.add_argument("--connections", type=int, default=30000)
parser.add_argument("--cmd-timeout", type=int, default=5, help="per-command timeout seconds")
args = parser.parse_args()

import redis

lock = threading.Lock()
barrier = threading.Barrier(threading.active_count() + 1)
stop_barrier = None
totals = {"ok": 0, "fail": 0, "ping_ok": 0, "info_ok": 0, "cinfo_ok": 0, "cslots_ok": 0}
times = {"ping": 0.0, "info": 0.0, "cluster_info": 0.0, "cluster_slots": 0.0}
first_errors = {"ping": [], "info": [], "cluster_info": [], "cluster_slots": []}

def timed_cmd(r, name, fn):
    t0 = time.time()
    try:
        r.execute_command(*fn) if isinstance(fn, tuple) else fn()
    finally:
        elapsed = time.time() - t0
        with lock:
            times[name] += elapsed

def worker(batch_size):
    conns = []
    for _ in range(batch_size):
        try:
            r = redis.Redis(host=args.host, port=args.port, password=args.password,
                            socket_timeout=args.cmd_timeout, socket_connect_timeout=args.cmd_timeout,
                            decode_responses=True)
            conns.append(r)
        except Exception:
            with lock:
                totals["fail"] += 1
    barrier.wait()
    for r in conns:
        cmds = [
            ("ping", lambda: r.ping()),
            ("info", lambda: r.info()),
            ("cluster_info", ("CLUSTER", "INFO")),
            ("cluster_slots", ("CLUSTER", "SLOTS")),
        ]
        all_ok = True
        for cname, fn in cmds:
            try:
                timed_cmd(r, cname, fn)
            except Exception as e:
                all_ok = False
                with lock:
                    key = f"{cname}_ok" if cname in ("ping", "info") else \
                          f"cinfo_ok" if cname == "cluster_info" else "cslots_ok"
                if len(first_errors[cname]) < 3:
                    first_errors[cname].append(str(e)[:100])
                break
        with lock:
            if all_ok:
                totals["ok"] += 1
                totals["ping_ok"] += 1
                totals["info_ok"] += 1
                totals["cinfo_ok"] += 1
                totals["cslots_ok"] += 1
            else:
                totals["fail"] += 1
    stop_barrier.wait()
    for r in conns:
        try: r.connection_pool.disconnect()
        except Exception: pass

total = args.connections
batch = min(500, total)
thread_count = max(1, total // batch)
barrier = threading.Barrier(thread_count + 1)
stop_barrier = threading.Barrier(thread_count + 1)

print(f"Connecting {total} clients ({thread_count} threads x {batch} each, timeout={args.cmd_timeout}s)...")
threads = []
t0 = time.time()
for _ in range(thread_count):
    t = threading.Thread(target=worker, args=(batch,))
    t.start()
    threads.append(t)
barrier.wait()
t1 = time.time()
print(f"All connected in {t1 - t0:.1f}s. Running commands...")
stop_barrier.wait()
for t in threads:
    t.join()
t2 = time.time()

print(f"--- Results ---")
print(f"connections: total={total} ok={totals['ok']} fail={totals['fail']}")
for key in ("ping_ok", "info_ok", "cinfo_ok", "cslots_ok"):
    label = key.replace("_ok", "").replace("cinfo", "CLUSTER INFO").replace("cslots", "CLUSTER SLOTS").replace("ping", "PING").replace("info", "INFO")
    print(f"  {label}: {totals[key]}/{total} ok")
    if first_errors.get(key.replace("_ok", "").replace("cinfo", "cluster_info").replace("cslots", "cluster_slots")):
        for e in first_errors[key.replace("_ok", "").replace("cinfo", "cluster_info").replace("cslots", "cluster_slots")][:3]:
            print(f"    err: {e}")
print(f"timing: total={t2 - t0:.1f}s connect={t1 - t0:.1f}s cmds={t2 - t1:.1f}s")
if totals["fail"] > 0:
    print(f"FAIL: {totals['fail']} failures")
    sys.exit(1)
print("PASS")
