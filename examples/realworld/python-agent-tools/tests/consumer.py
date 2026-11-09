"""Run solely against an installed wheel, with no source/toolchain imports."""
import json
import pathlib
import shutil
import sys
import sengoo_agent_tools as tools


def verify(iterations=200):
    for name in ("sgc","sgpm","sgfmt","clang","clang-cl","cargo","rustc","gcc","cl"):
        assert shutil.which(name) is None, ("build tool on consumer PATH",name)
    assert "sengoo_python" not in sys.modules
    schema={"type":"object","properties":{"path":{"type":"string","minLength":1},"count":{"type":"integer","enum":[9223372036854775807]}},"required":["path","count"],"additionalProperties":False}
    encoded=json.dumps(schema,ensure_ascii=False)
    arguments=json.dumps({"path":"中文/😀.md","count":9223372036854775807},ensure_ascii=False)
    assert tools.require_valid_tool(encoded,arguments) is True
    report=json.loads(tools.validation_report(encoded,arguments));assert report["valid"] and report["stage"]==""
    canonical=tools.canonical_arguments(encoded,arguments)
    assert json.loads(canonical)==json.loads(arguments) and "9223372036854775807" in canonical
    invalid=json.dumps({"path":"","count":9223372036854775807})
    assert not json.loads(tools.validation_report(encoded,invalid))["valid"]
    def fails(code,function,*values):
        try:function(*values)
        except tools.SengooError as error:assert error.code==code,(code,error.code)
        else:raise AssertionError(("expected SengooError",function.__name__))
    fails(2,tools.require_valid_tool,encoded,invalid)
    fails(10,tools.require_valid_tool,'{"unknownKeyword":true}','{}')
    for value in ('{"path":"before\\u0000after","count":9223372036854775807}', '{"path\\u0000suffix":"hidden","count":9223372036854775807}'):
        fails(2,tools.require_valid_tool,encoded,value)
    for index in (0,1):
        values=[encoded,arguments];values[index]+="\0hidden"
        try:tools.require_valid_tool(*values)
        except ValueError:pass
        else:raise AssertionError("literal NUL must fail Python conversion")
    fails(13,tools.require_valid_tool,'{"type":"object"}','{"text":"'+'x'*32768+'"}')
    fails(2,tools.require_valid_tool,encoded,json.dumps({"path":"ok","count":9223372036854775806}))
    baseline=(tools.native_live_strings(),tools.native_live_buffers(),tools.native_live_documents())
    for index in range(iterations):
        assert tools.require_valid_tool(encoded,arguments)
        assert json.loads(tools.canonical_arguments(encoded,arguments))["count"]==9223372036854775807
        assert not json.loads(tools.validation_report(encoded,invalid))["valid"]
        fails(2,tools.require_valid_tool,encoded,invalid)
        fails(10,tools.require_valid_tool,'{"unknownKeyword":true}','{}')
        assert (tools.native_live_strings(),tools.native_live_buffers(),tools.native_live_documents())==baseline,("native resource drift",index,baseline)
    directory=pathlib.Path(tools.__file__).parent
    assert (directory/"__init__.pyi").is_file() and (directory/"_bindings.json").is_file()
    return {"result":"passed","iterations":iterations,"resource_baseline":baseline,"checks":["installed-wheel-without-backend-or-compiler","real-schema-accept-reject","unicode-exact-i64","typed-error-and-nul-boundaries","repeated-success-failure-resource-stability"]}


if __name__=="__main__":print(json.dumps(verify(),ensure_ascii=False))
