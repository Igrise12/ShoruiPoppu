"""Run with Vite on port 1420 and WebKitWebDriver on port 4448.

    npm run dev -- --host 127.0.0.1
    WebKitWebDriver --port=4448
    python3 tests/viewer-webkit.py
"""
import json, time, urllib.request
base='http://127.0.0.1:4448'
def req(path, body=None):
    data=None if body is None else json.dumps(body).encode()
    with urllib.request.urlopen(urllib.request.Request(base+path, data=data, headers={'Content-Type':'application/json'}), timeout=20) as r: return json.load(r)['value']
sid=req('/session',{'capabilities':{'alwaysMatch':{'browserName':'MiniBrowser'}}})['sessionId']
def call(path,body=None): return req('/session/'+sid+path,body)
def execute(script,*args): return call('/execute/sync',{'script':script,'args':list(args)})
def wait_for(script):
    for _ in range(150):
        value=execute(script)
        if value: return value
        time.sleep(.1)
    raise AssertionError('Timed out: '+script)
def load(mode):
    call('/url',{'url':'http://127.0.0.1:1420/tests/viewer-fixture.html?mode='+mode})
    wait_for('return document.querySelectorAll(".preview").length > 0')
try:
    load('docx')
    data=wait_for('const p=[...document.querySelectorAll(".doc-pages > .doc-page")]; return p.length>1 && p.map(x=>x.innerText).join("").includes("LAST END") ? {pages:p.length,text:p.map(x=>x.textContent).join(""),rects:p.map(x=>({top:x.getBoundingClientRect().top,bottom:x.getBoundingClientRect().bottom})),previews:document.querySelectorAll(".preview").length} : null')
    assert data['pages']==data['previews'],data
    assert all('ROW_'+str(i)+'_END' in data['text'] for i in range(35)),data
    assert all('word'+str(i)+' ' in data['text'] for i in range(200)),data
    assert all('cellword'+str(i)+' ' in data['text'] for i in range(120)),data
    assert execute('return document.querySelectorAll(".doc-pages img").length') == 1
    assert data['text'].index('BEFORE_BREAK')<data['text'].index('AFTER_BREAK')<data['text'].index('ROW_0_END'),data
    assert data['rects'][1]['top']-data['rects'][0]['bottom']>=19,data
    execute('document.querySelectorAll(".preview")[1].click()')
    wait_for('return document.querySelectorAll(".preview")[1].getAttribute("aria-current")==="true"')
    details=execute('return [...document.querySelectorAll(".doc-pages > .doc-page")].map(p=>{let w=document.createTreeWalker(p,NodeFilter.SHOW_TEXT),n,over=0;while(n=w.nextNode()){let r=document.createRange();r.selectNodeContents(n); if(r.getBoundingClientRect().bottom>p.getBoundingClientRect().bottom+1)over++} return {chars:p.textContent.length,over}})')
    assert all(d['over']==0 and d['chars']>0 for d in details),details
    assert execute('const p=[...document.querySelectorAll(".doc-pages > .doc-page")]; return p.findIndex(x=>x.textContent.includes("BEFORE_BREAK")) < p.findIndex(x=>x.textContent.includes("AFTER_BREAK"))')
    print('DOCX:',data['pages'],'pages; content, previews, spacing, and navigation passed')
    load('docxlarge')
    large=wait_for('const p=[...document.querySelectorAll(".doc-pages > .doc-page")]; return p.length>10 && p[p.length-1].textContent.includes("Paragraph 299") ? {pages:p.length,loaded:document.querySelectorAll(".preview-image .doc-page").length} : null')
    assert large['loaded'] < large['pages'],large
    print('DOCX large:',large['pages'],'pages; previews load on demand')
    load('pptx')
    wait_for('return document.querySelectorAll(".preview-image .slide").length===3')
    execute('document.querySelectorAll(".preview")[1].click(); document.querySelectorAll(".preview")[2].click()')
    wait_for('return document.querySelector(".status")?.textContent==="Slide 3 of 3"')
    assert execute('return document.querySelectorAll(".preview")[2].getAttribute("aria-current")')=='true'
    load('pptxfail')
    wait_for('return document.querySelectorAll(".preview-image")[1].textContent==="Preview unavailable"')
    execute('document.querySelectorAll(".preview")[1].click()')
    wait_for('return document.querySelector(".status")?.textContent==="Slide 2 of 3"')
    print('PPTX: previews, failed preview, and rapid navigation passed')
    load('pdf')
    wait_for('return document.querySelectorAll(".pdf-page canvas").length>0 && document.querySelectorAll(".preview-image canvas").length>0')
    data=execute('const p=[...document.querySelectorAll(".pdf-page")]; return {count:p.length,gap:p[1].getBoundingClientRect().top-p[0].getBoundingClientRect().bottom}')
    assert data['count']==2 and data['gap']>=19,data
    execute('document.querySelectorAll(".preview")[1].click()')
    wait_for('return document.querySelectorAll(".preview")[1].getAttribute("aria-current")==="true"')
    execute('Array.from(document.querySelectorAll("button")).find(b=>b.getAttribute("aria-label")==="Zoom in").click()')
    wait_for('return document.querySelectorAll(".pdf-page canvas").length>0')
    assert execute('const p=[...document.querySelectorAll(".pdf-page")]; return p[1].getBoundingClientRect().top-p[0].getBoundingClientRect().bottom') >= 19
    print('PDF: pages, previews, zoom, spacing, and navigation passed')
finally:
    request=urllib.request.Request(base+'/session/'+sid, method='DELETE')
    urllib.request.urlopen(request, timeout=10).close()
