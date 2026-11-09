"""Public CLI offline contract; only synthetic temporary records are used."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(); parser.add_argument("executable", type=Path)
    parser.add_argument("--report", type=Path, required=True); args = parser.parse_args()
    executable = args.executable.resolve(strict=True)
    root = Path(tempfile.mkdtemp(prefix="sgeval-离线-")); checks = []

    def write(name, value):
        path = root / name
        path.write_text(json.dumps(value, ensure_ascii=False), encoding="utf-8")
        return path

    def invoke(*arguments, code=0):
        result = subprocess.run([str(executable), *map(str, arguments)], cwd=root,
            capture_output=True, text=True, encoding="utf-8", timeout=30)
        assert result.returncode == code, (arguments, result.returncode, result.stdout, result.stderr)
        assert "SYNTHETIC-KEY-ONLY" not in result.stdout + result.stderr
        return result

    evidence = {"schema":1,"scope":"offline-synthetic-public-cli",
        "executable_sha256":hashlib.sha256(executable.read_bytes()).hexdigest(),"checks":checks}
    try:
        source = root / "input.jsonl"
        events = [
            {"type":"request","headers":{"aUtHoRiZaTiOn":"Bearer SYNTHETIC-KEY-ONLY","X-API-KEY":"unlisted-field-secret"},"payload":{"nested":[{"password":"never keep"}]},"text":"Hello SYNTHETIC-KEY-ONLY 中文😀"},
            {"type":"tool","name":"run_command","arguments":{"command":"echo side-effect > should-not-exist.txt"},"value":9223372036854775807},
            {"type":"result","success":True,"text":"done"},
        ]
        source.write_text("\r\n".join(json.dumps(x,ensure_ascii=False) for x in events)+"\r\n",encoding="utf-8")
        redactions = write("redactions.json",{"secrets":["SYNTHETIC-KEY-ONLY"]})
        recorded = root / "recorded.jsonl"; invoke("record",source,recorded,redactions)
        content = recorded.read_text(encoding="utf-8")
        assert all(x not in content for x in ("SYNTHETIC-KEY-ONLY","unlisted-field-secret","never keep"))
        rows = [json.loads(line) for line in content.splitlines()]
        assert [r["sequence"] for r in rows] == [0,1,2]
        assert rows[0]["event"]["text"] == "Hello [REDACTED] 中文😀"
        assert rows[0]["event"]["payload"]["nested"][0]["password"] == "[REDACTED]"
        assert rows[1]["event"]["value"] == 9223372036854775807
        checks.append("recursive-sensitive-fields-and-literal-redaction-exact-i64-unicode-crlf")
        spec={"schema":1,"event_count":3,"types":["request","tool","result"],"checks":[
            {"event":0,"path":["headers","aUtHoRiZaTiOn"],"equals":"[REDACTED]"},
            {"event":1,"path":["value"],"equals":9223372036854775807},
            {"event":2,"path":["success"],"equals":True}],"absent_strings":["SYNTHETIC-KEY-ONLY"]}
        assertions = write("assertions.json",spec); report=root/"report.json"
        invoke("replay",recorded,assertions,report)
        first=report.read_bytes(); assert json.loads(first)["passed"]
        second=root/"report-again.json"; invoke("replay",recorded,assertions,second)
        assert second.read_bytes()==first
        assert not (root/"should-not-exist.txt").exists()
        checks.append("deterministic-replay-report-never-executes-recorded-command")
        badspec=dict(spec,event_count=4); bad=write("bad-assertions.json",badspec)
        failed=root/"failed.json"; invoke("replay",recorded,bad,failed,code=1)
        assert json.loads(failed.read_text(encoding="utf-8"))["passed"] is False
        checks.append("failed-assertion-nonzero-and-failed-report")
        for name,invalid in (("gap",content.replace('"sequence":0','"sequence":9',1)),("bad-json","{not-json"),("bad-utf8",b'\xff'),("bad-version",content.replace('"schema":1','"schema":99',1))):
            path=root/(name+".jsonl")
            path.write_bytes(invalid if isinstance(invalid,bytes) else invalid.encode())
            target=root/(name+".report.json"); invoke("replay",path,assertions,target,code=2)
            assert not target.exists()
        checks.append("invalid-json-utf8-version-sequence-rejected-without-success-report")
        unknown=write("unsupported.json",dict(spec,execute=True)); unknown_report=root/"unknown.json"
        invoke("replay",recorded,unknown,unknown_report,code=2); assert not unknown_report.exists()
        bad_input=root/"raw-bad.jsonl"; bad_input.write_text('{"type":"text"}\n{broken',encoding="utf-8")
        not_created=root/"not-created.jsonl"; invoke("record",bad_input,not_created,redactions,code=2); assert not not_created.exists()
        before=recorded.read_bytes(); invoke("record",source,recorded,redactions,code=2); assert recorded.read_bytes()==before
        checks.append("unknown-assertion-no-partial-recording-existing-output-preserved")
        large=root/"large-raw.jsonl"
        large.write_text(json.dumps({"type":"text_delta","text":"x"*1048576}),encoding="utf-8")
        limited=root/"large-recorded.jsonl"; invoke("record",large,limited,code=2); assert not limited.exists()
        missing=write("missing-path.json",dict(spec,checks=[{"event":0,"path":["absent"],"equals":1}]))
        invalid_report=root/"missing-path-report.json"
        invoke("replay",recorded,missing,invalid_report,code=2); assert not invalid_report.exists()
        after=report.read_bytes(); invoke("replay",recorded,assertions,report,code=2); assert report.read_bytes()==after
        checks.append("explicit-line-capacity-missing-path-and-report-overwrite-rejected")
        # Reject exact owned NUL bytes before any C-string view can hide data.
        nul_events = [
            {"type":"text_delta","text":"SYNTHETIC-KEY-ONLY\x00after"},
            {"type":"text_delta","text":"before\x00SYNTHETIC-KEY-ONLY"},
            {"type":"text_delta","headers":{"Authorization\x00suffix":"SYNTHETIC-KEY-ONLY"}},
            {"type":"text_delta","headers":{"\x00Authorization":"SYNTHETIC-KEY-ONLY"}},
            {"type":"text_delta","payload":[{"nested":"before\x00SYNTHETIC-KEY-ONLY"}]},
        ]
        for index,event in enumerate(nul_events):
            path=root/f"nul-event-{index}.jsonl";path.write_text(json.dumps(event)+"\n",encoding="utf-8")
            output=root/f"nul-record-{index}.jsonl";invoke("record",path,output,redactions,code=2)
            assert not output.exists()
        raw_nul=root/"raw-nul.jsonl";raw_nul.write_bytes(source.read_bytes()+b'\x00'+b'{"type":"hidden"}')
        raw_output=root/"raw-nul-out.jsonl";invoke("record",raw_nul,raw_output,redactions,code=2);assert not raw_output.exists()
        bad_redactions=write("nul-redactions.json",{"secrets":["before\x00SYNTHETIC-KEY-ONLY"]})
        out=root/"nul-redactions-out.jsonl";invoke("record",source,out,bad_redactions,code=2);assert not out.exists()
        nul_specs=[dict(spec,types=["request\x00suffix","tool","result"]),
            dict(spec,checks=[{"event":2,"path":["text"],"equals":"done\x00SYNTHETIC-KEY-ONLY"}]),
            dict(spec,checks=[{"event":2,"path":["text\x00suffix"],"equals":"done"}]),
            dict(spec,absent_strings=["absent\x00SYNTHETIC-KEY-ONLY"]),
            {**spec,"schema\x00suffix":1}]
        for index,invalid in enumerate(nul_specs):
            path=write(f"nul-spec-{index}.json",invalid);output=root/f"nul-report-{index}.json"
            invoke("replay",recorded,path,output,code=2);assert not output.exists()
        nul_recording=root/"nul-recording.jsonl"
        altered=[dict(r) for r in rows];altered[2]={**rows[2],"event":{**rows[2]["event"],"text":"done\x00SYNTHETIC-KEY-ONLY"}}
        nul_recording.write_text("\n".join(json.dumps(r) for r in altered)+"\n",encoding="utf-8")
        nul_report=root/"nul-recording-report.json";invoke("replay",nul_recording,assertions,nul_report,code=2);assert not nul_report.exists()
        checks.append("nul-raw-escaped-values-keys-redactions-paths-and-assertions-rejected-before-narrowing")
        evidence["result"]="passed"
    except BaseException as error:
        evidence["result"]="failed";evidence["error"]=repr(error);raise
    finally:
        args.report.parent.mkdir(parents=True,exist_ok=True)
        args.report.write_text(json.dumps(evidence,ensure_ascii=False,indent=2),encoding="utf-8")
        for path in root.iterdir():
            assert path.is_file() and path.resolve().is_relative_to(root.resolve());path.unlink()
        root.rmdir()
    print(json.dumps(evidence,ensure_ascii=False,indent=2))


if __name__=="__main__":main()
