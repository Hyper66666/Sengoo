"""An independent peer stops reading stdout, then cancels via stdin."""
import json
import queue
import subprocess
import threading
import time


def exercise(server, deadline=False):
    process = subprocess.Popen([str(server)] + (["deadline"] if deadline else []),
                               stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    messages = queue.Queue()
    retained = []
    def diagnostics():
        for line in process.stderr:
            retained.append(line.decode("utf-8", "replace").rstrip())
            messages.put(retained[-1])
    thread = threading.Thread(target=diagnostics)
    thread.start()
    def send(value):
        process.stdin.write((json.dumps(value) + "\n").encode())
        process.stdin.flush()
    try:
        send({"jsonrpc":"2.0", "id":1, "method":"initialize", "params":{
            "protocolVersion":"2025-11-25", "capabilities":{},
            "clientInfo":{"name":"independent-blocked-peer", "version":"1"}}})
        # Read exactly the initialization line, then deliberately stop consuming.
        initialized = queue.Queue()
        first_line = threading.Thread(target=lambda: initialized.put(process.stdout.readline()))
        first_line.start()
        assert json.loads(initialized.get(timeout=10))["id"] == 1
        first_line.join(timeout=1)
        send({"jsonrpc":"2.0", "method":"notifications/initialized"})
        started = time.monotonic()
        send({"jsonrpc":"2.0", "id":"blocked", "method":"tools/call", "params":{
            "name":"flood", "arguments":{}, "_meta":{"progressToken":"blocked-progress"}}})
        assert messages.get(timeout=10) == "output-backpressure-observed"
        if not deadline:
            # Ensure a real full pipe rather than merely an asynchronously queued write.
            time.sleep(0.1)
            send({"jsonrpc":"2.0", "method":"notifications/cancelled", "params":{"requestId":"blocked"}})
        else:
            # Allow one complete native write, then stop again. Progress must not
            # restart the original deadline while the remaining output is blocked.
            progressed = queue.Queue()
            partial_reader = threading.Thread(target=lambda: progressed.put(process.stdout.read(65536)))
            partial_reader.start()
            assert len(progressed.get(timeout=2)) == 65536
            partial_reader.join(timeout=1)
        process.wait(timeout=5)
        thread.join(timeout=2)
        assert process.returncode == 0, retained
        expected = "original-deadline-observed-and-joined" if deadline else "cancel-observed-and-joined"
        assert expected in retained and "protocol-resources-at-baseline" in retained, retained
        if deadline:
            assert "output-progress-within-deadline" in retained, retained
        duration = time.monotonic() - started
        assert duration < 4
        if deadline:
            assert 0.4 <= duration < 3, duration
        return {"case":"deadline" if deadline else "cancel", "seconds":duration,
                "exit_code":process.returncode, "diagnostics":retained}
    finally:
        if process.poll() is None:
            process.kill()
            process.wait(timeout=5)
        for pipe in (process.stdin, process.stdout, process.stderr):
            pipe.close()
        thread.join(timeout=2)


def verify(server):
    # Repetition catches resource/lifecycle regressions across independently owned hosts.
    return [exercise(server, deadline) for _ in range(3) for deadline in (False, True)]


if __name__ == "__main__":
    import sys
    print(json.dumps(verify(sys.argv[1]), indent=2))
