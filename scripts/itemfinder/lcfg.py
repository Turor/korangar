# minimal libconfig span scanner for Hercules DBs
import re
def skip_ws(t,i):
    n=len(t)
    while i<n:
        if t.startswith('//',i): i=t.find('\n',i); i=n if i<0 else i
        elif t.startswith('/*',i): i=t.find('*/',i)+2
        elif t[i].isspace(): i+=1
        else: break
    return i
def skip_value(t,i):
    """return end index of a value starting at i (block/list/scalar/script)"""
    n=len(t)
    if t.startswith('<"',i):
        return t.find('">',i+2)+2
    if t[i] in '{([':
        close={'{':'}','(':')','[':']'}; st=[close[t[i]]]; i+=1
        while i<n and st:
            if t.startswith('//',i): i=t.find('\n',i); continue
            if t.startswith('/*',i): i=t.find('*/',i)+2; continue
            if t.startswith('<"',i): i=t.find('">',i+2)+2; continue
            c=t[i]
            if c=='"':
                i+=1
                while t[i]!='"':
                    i+= 2 if t[i]=='\\' else 1
                i+=1; continue
            if c in '{([': st.append(close[c])
            elif c in '})]': st.pop()
            i+=1
        return i
    if t[i]=='"':
        i+=1
        while t[i]!='"': i+= 2 if t[i]=='\\' else 1
        return i+1
    m=re.compile(r'[^\s,;}\)\]]+').match(t,i); return m.end()
def fields(t,s,e):
    """parse settings inside block body t[s:e]; return list of (key,kstart,vstart,vend)"""
    out=[]; i=s
    while True:
        i=skip_ws(t,i)
        if i>=e: break
        if t[i] in ',;': i+=1; continue
        m=re.compile(r'[A-Za-z0-9_@\-\.]+').match(t,i)
        if not m: i+=1; continue
        k=m.group(0); j=skip_ws(t,m.end())
        if t[j] not in ':=': i=m.end(); continue
        j=skip_ws(t,j+1); ve=skip_value(t,j)
        out.append((k,i,j,ve)); i=ve
    return out
def list_entries(t,s,e):
    """entries {..} inside a list body -> list of (start,end)"""
    out=[]; i=s
    while True:
        i=skip_ws(t,i)
        if i>=e: break
        if t[i]=='{':
            j=skip_value(t,i); out.append((i,j)); i=j
        else: i+=1
    return out
def norm(v): return re.sub(r'\s+',' ',re.sub(r'//[^\n]*','',v)).strip()
