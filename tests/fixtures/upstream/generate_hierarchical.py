# Copyright (c) Meta Platforms, Inc. and affiliates.
# Fixture harness for original MIT-licensed functions; see LICENSE-META.
"""Execute selected upstream AST bodies, without loading the agent runtime.
Usage: python3 tests/fixtures/upstream/generate_hierarchical.py /path/to/cca-checkout
Requires PyYAML and Pydantic. Does not execute the full Confucius agent.
"""
import ast
import difflib
import hashlib
import json
import sys
import tempfile
import types
from pathlib import Path

root = Path(sys.argv[1])
base = 'confucius/orchestrator/extensions/'
paths = {k: base + v for k, v in {
    'types': 'memory/hierarchical/types.py',
    'utils': 'memory/hierarchical/utils.py',
    'extension': 'memory/hierarchical/extension.py',
    'file': 'file/utils.py',
}.items()}
source = {k: (root / v).read_text() for k, v in paths.items()}
ns = {}
exec(compile(source['types'], paths['types'], 'exec'), ns)
utree = ast.parse(source['utils'])
utree.body = [n for n in utree.body if not isinstance(n, ast.ImportFrom) or n.level == 0]
exec(compile(utree, paths['utils'], 'exec'), ns)
ns['utils'] = types.SimpleNamespace(**ns)
# Keep the upstream scanner body after session directory resolution.
ext = ast.parse(source['extension'])
cls = next(n for n in ext.body if isinstance(n, ast.ClassDef) and n.name == 'HierarchicalMemoryExtension')
method = next(n for n in cls.body if isinstance(n, ast.AsyncFunctionDef) and n.name == '_load_memory_from_filesystem')
scanner = ast.FunctionDef(name='scan', args=ast.arguments(posonlyargs=[], args=[ast.arg(arg='base_dir')], kwonlyargs=[], kw_defaults=[], defaults=[]), body=method.body[2:], decorator_list=[])
exec(compile(ast.fix_missing_locations(ast.Module(body=[scanner], type_ignores=[])), paths['extension'], 'exec'), ns)
ftree = ast.parse(source['file'])
selected = {'view_file_content', 'view_directory', '_remove_empty_lines', '_is_full_line_match', '_replace_lines_in_original_content'}
ftree.body = [n for n in ftree.body if isinstance(n, ast.FunctionDef) and n.name in selected]
from textwrap import dedent
ns['dedent'] = dedent
exec(compile(ftree, paths['file'], 'exec'), ns)

cases = []
def run(name, function, args):
    try:
        result = ns[function](**args)
        if isinstance(result, tuple):
            result = list(result)
        expected = {'value': result}
    except Exception as exc:
        expected = {'error_type': type(exc).__name__, 'message': str(exc)}
    cases.append({'name': name, 'function': function, 'input': args, 'expected': expected, 'derivation': 'executed_upstream_ast'})

for name, content in [
    ('plain', '# Note\n'), ('tags', '---\ntags:\n- rust\n- memory\n---\n# Note\n'),
    ('note_taker_metadata', '---\nid: note\ntitle: Note\nkeywords: [rust]\n---\nBody\n'),
    ('invalid_yaml', '---\ntags: [\n---\nBody'), ('wrong_type', '---\ntags: rust\n---\nBody'),
    ('empty_tags', '---\ntags: []\n---\nBody'), ('no_closing', '---\ntags: [rust]'),
    ('crlf', '---\r\ntags: [rust]\r\n---\r\nBody'),
    ('nonstring_tag', '---\ntags: [12, true]\n---\nBody'),
]: run(name, 'parse_frontmatter', {'content': content})
for name,tags in [('no_tags', []), ('tag_serialization', ['rust','memory']), ('quoted_tags',['true','a: b','café']), ('yaml11_ambiguous_strings',['yes','on','2026-10-27','line\u0085next','line\u2028next'])]:
    run(name,'create_content_with_frontmatter',{'content':'Body\n','tags':tags})
for name,args in [
    ('empty', {'content':''}), ('trailing_newline', {'content':'a\n'}),
    ('partial',{'content':'α\nβ\nγ','start_line':2,'end_line':2}),
    ('end_sentinel',{'content':'a\nb','end_line':-1}),
    ('beyond_eof',{'content':'a','start_line':4,'end_line':5}),
    ('zero_start',{'content':'a','start_line':0}),
    ('reversed',{'content':'a\nb','start_line':2,'end_line':1}),
    ('raw_frontmatter',{'content':'---\ntags: [rust]\n---\nBody','start_line':2,'end_line':4}),
]: run(name,'view_file_content',dict(start_line=None,end_line=None,max_view_lines=None,**{}) | args)

with tempfile.TemporaryDirectory() as tmp:
    directory = Path(tmp)
    files = {'a/b/note.md':'---\ntags: [rust]\n---\nContains [ and café\n',
             'z.md':'x'*101, '.hidden.md':'Hidden', 'ignored.txt':'ignored'}
    for name,content in files.items():
        p=directory/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(content)
    (directory/'empty').mkdir()
    memory=ns['scan'](directory)
    def node_json(node):
        return {'path':str(node.path.relative_to(directory)), 'name':node.name,'content':node.content,'tags':node.tags,'children':[node_json(c) for c in node.children]}
    cases.append({'name':'filesystem_tree','function':'_load_memory_from_filesystem','input':{'files':files,'directories':['empty']},'expected':{'value':[node_json(n) for n in memory.nodes]},'derivation':'executed_upstream_ast_after_base_dir_resolution'})
    for name,filters in [('all',{}),('cross_directory_star',{'path_pattern':'a/*'}),('md_suffix_removed',{'path_pattern':'a/*.md'}),('invalid_regex_literal',{'content_pattern':'['}),('case_insensitive',{'content_pattern':'CAFÉ'}),('all_tags',{'tags':['rust','missing']}),('zero_limit',{'max_results':0}),('one_limit',{'max_results':1}),('python_lookbehind',{'content_pattern':'(?<=Contains )\\['})]:
        args={'path_pattern':None,'content_pattern':None,'tags':None,'max_results':20}|filters
        results=[]
        ns['collect_matching_nodes'](memory.nodes,**args,results=results,base_dir=directory)
        cases.append({'name':name,'function':'collect_matching_nodes','input':args,'tree_fixture':'filesystem_tree','expected':{'value':results},'derivation':'executed_upstream_ast'})

# Execute only deterministic normalization/replacement bodies, not unavailable rapidfuzz.
for name,original,find,replacement in [('full_line_spaces','before\n   \nend','before\n\nend','changed'),('substring','prefix before\n   \nend','before','changed')]:
    run(name,'_replace_lines_in_original_content',{'file_content':ns['_remove_empty_lines'](original),'find_text':ns['_remove_empty_lines'](find),'original_file_content':original,'replace_text':replacement})

# Full edit function execution uses original RapidFuzz at the diagnostic boundary.
from rapidfuzz import fuzz, __version__ as rapidfuzz_version
ns.update(fuzz=fuzz, difflib=difflib, DEFAULT_SIMILARITY_THRESHOLD=0.8)
edit_names = {'ChunkWithSimilarity', 'find_matched_chunks_with_similarity', '_validate_uniqueness', 'replace_in_file'}
edit_tree = ast.parse(source['file'])
edit_tree.body = [n for n in edit_tree.body if isinstance(n, (ast.FunctionDef, ast.ClassDef)) and n.name in edit_names]
exec(compile(edit_tree, paths['file'], 'exec'), ns)
for name,content,old,new in [
    ('edit_full_line_blank', 'before\n   \nend', 'before\n\nend', 'changed'),
    ('edit_substring_blank', 'prefix before\n   \nend', 'before', 'changed'),
    ('edit_overlapping', 'aaa', 'aa', 'x'),
    ('edit_crlf', 'a\r\nb\r\n', 'b', 'x'),
    ('edit_empty', 'abc', '', 'x'),
    ('edit_not_found', 'abc', 'uvwxyz', 'x'),
    ('edit_near_match', 'abcdefghij', 'abcdefghiX', 'x'),
]:
    with tempfile.TemporaryDirectory() as temp:
        path=Path(temp)/'note.md'; path.write_text(content)
        try:
            ns['replace_in_file'](path,old,new,require_line_num=False)
            result={'value':path.read_text()}
        except Exception as error:
            result={'error_type':type(error).__name__, 'unchanged':path.read_text()==content}
        cases.append({'name':name,'function':'replace_in_file','input':{'content':content,'old_str':old,'new_str':new},'expected':result,'derivation':'executed_upstream_ast_with_rapidfuzz_'+rapidfuzz_version})

import fnmatch
for pattern in ['a/*','[ab]','[!a]','[z-a]','[z-a-b]','[a--b]','[--0]','[]]','[!]]','[[]','[','a?','**','a*b*c','[a-b-c]']:
    for text in ['a','b','c','-','[',']','a/b','a/b/c','abc','axbyc','']:
        cases.append({'name':pattern+' :: '+text,'function':'fnmatch','input':{'pattern':pattern,'text':text},'expected':{'value':fnmatch.fnmatch(text,pattern)},'derivation':'executed_python_fnmatch'})

compatibility = []
for name, content in [
    ('unquoted_yes', '---\ntags: [yes]\n---\nBody'),
    ('unquoted_on', '---\ntags: [on]\n---\nBody'),
    ('unquoted_date', '---\ntags: [2026-10-27]\n---\nBody'),
    ('quoted_yaml11_scalars', '---\ntags: ["yes", "on", "2026-10-27"]\n---\nBody'),
    ('duplicate_tags', '---\ntags: [first]\ntags: [second]\n---\nBody'),
]:
    tags, body = ns['parse_frontmatter'](content)
    try:
        node = ns['MemoryNode'](path=Path('note.md'),name='note.md',tags=tags,content=body)
        expected = {'value': [node.tags, node.content]}
    except Exception:
        expected = {'error_type': 'ValidationError'}
    compatibility.append({'name':name,'input':content,'expected':expected,'derivation':'executed_upstream_parse_plus_MemoryNode'})

output={'compatibility_cases':compatibility,'upstream_commit' :'bdeb74b3d828bfbb2c61030c6201fa9da44e69d0','license':'MIT; LICENSE-META','execution':'Original AST function bodies; no full Analect runtime. Scanner session base-dir resolution replaced by a temporary input directory. Edit functions executed with RapidFuzz (version recorded per case).','source_sha256':{paths[k]:hashlib.sha256(v.encode()).hexdigest() for k,v in source.items()},'cases':cases}
Path(__file__).with_name('hierarchical.json').write_text(json.dumps(output,ensure_ascii=False,indent=2)+'\n')
print(f'Wrote {len(cases)} executed cases')
