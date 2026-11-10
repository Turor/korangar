"""Item Finder sources: python3 genitemsources.py <hercules root> <mapnametable.txt|-> <out.json> [fallback-v2.json]

Drops (server rates applied), spawns (loaded npc files only) and quest-script rewards per item.
Writes schema version 3 (`v`: 3), which `parse_sources` in korangar reads
(drops, mobs, quests, penalty, rates).

When `<hercules root>/npc/re/scripts_main.conf` is missing, pass the previous
itemsources JSON as the fourth argument. Quest file lists and mob spawn rows are
copied from that snapshot. Drops, mob stats, penalty, and rates still come from
the Hercules db/conf tree. A full hercules-dev checkout regenerates quests and
spawns from the NPC scripts and ignores the fallback for those fields.
"""
import re,sys,os,json,collections
sys.path.insert(0,os.path.dirname(__file__))
from geniteminfo2 import items
from lcfg import fields,list_entries

def conf_vals(root,paths):
    """battle conf: later files override earlier"""
    v={}
    for p in paths:
        t=re.sub(r'//.*','',open(os.path.join(root,p),encoding='latin-1').read())
        for k,x in re.findall(r'^\s*(\w+)\s*:\s*(-?\d+|true|false|yes|no)',t,re.M): v[k]=x
    return v

def loaded_npc(root):
    removed=set(); files=[]
    def walk(conf,into):
        t=open(os.path.join(root,conf),encoding='latin-1').read()
        t=re.sub(r'/\*.*?\*/','',t,flags=re.S); t=re.sub(r'//.*','',t)
        for m in re.finditer(r'@include\s+"([^"]+)"|"([^"]+\.txt)"',t):
            if m.group(1):
                lst=removed if 'removed' in m.group(1) else into
                if os.path.exists(os.path.join(root,m.group(1))): walk(m.group(1),lst)
            else: (into.add if isinstance(into,set) else into.append)(m.group(2))
    walk('npc/re/scripts_main.conf',files)
    return [f for f in dict.fromkeys(files) if f not in removed and os.path.exists(os.path.join(root,f))]

def penalty(root):
    rows=collections.defaultdict(dict)
    for l in open(os.path.join(root,'db/re/level_penalty.txt')):
        m=re.match(r'^\s*2,(\d+),(-?\d+),(\d+)',l)
        if m: rows[int(m.group(1))][int(m.group(2))]=int(m.group(3))
    return {k:sorted(v.items()) for k,v in rows.items()}

def main():
    root,mnt,outp=sys.argv[1:4]
    fallback=sys.argv[4] if len(sys.argv)>4 and sys.argv[4] not in ('-', '') else None
    rate_paths=[p for p in ('conf/map/battle/drops.conf','conf/import/battle.conf') if os.path.exists(os.path.join(root,p))]
    bc=conf_vals(root,rate_paths) if rate_paths else {}
    I=lambda k,d=100: int(bc.get(k,d))
    # items: aegis -> (id,type)
    aeg={}; typ={}
    db_paths=[os.path.join(root,'db/re/item_db.conf')]
    db2=os.path.join(root,'db/item_db2.conf')
    if os.path.exists(db2): db_paths.append(db2)
    for f in [row for path in db_paths for row in items(path)]:
        i=int(f['Id']); a=f.get('AegisName','').strip('"')
        if a: aeg[a]=i
        if 'Type' in f: typ[i]=f['Type'].strip('"')
        elif 'CloneItem' in f: typ[i]=typ.get(int(f['CloneItem']) if f['CloneItem'].isdigit() else aeg.get(f['CloneItem'].strip('"')), 'IT_ETC')
    def adj(base,i,boss,mvp=False):
        if mvp: r,mn,mx=I('item_rate_mvp'),I('item_drop_mvp_min',1),I('item_drop_mvp_max',10000)
        else:
            t=typ.get(i,'IT_ETC')
            cat={'IT_HEALING':'heal','IT_USABLE':'use','IT_CASH':'use','IT_WEAPON':'equip','IT_ARMOR':'equip','IT_PETARMOR':'equip','IT_CARD':'card'}.get(t,'common')
            r=I('item_rate_'+cat+('_boss' if boss else '')); mn=I('item_drop_'+cat+'_min',1); mx=I('item_drop_'+cat+'_max',10000)
        rate=base*r//100 if r!=100 else base
        return max(mn,min(mx,rate))
    # mobs
    mobs={}; drops=collections.defaultdict(list)
    for p in ('db/re/mob_db.conf','db/mob_db2.conf'):
        mob_path=os.path.join(root,p)
        if not os.path.exists(mob_path): continue
        t=open(mob_path,encoding='utf-8',errors='replace').read()
        for k,ks,vs,ve in fields(t,0,len(t)):
            if t[vs]!='(': continue
            for s,e in list_entries(t,vs+1,ve-1):
                f={k:t[a:b] for k,_,a,b in fields(t,s+1,e-1)}
                mid=int(f['Id']); m=mobs.get(mid,{})
                if 'Name' in f: m['n']=f['Name'].strip('"')
                if 'Lv' in f: m['lv']=int(f['Lv'])
                boss=bool(re.search(r'Boss:\s*true',f.get('Mode',''))) if 'Mode' in f else m.get('boss',False)
                m['boss']=boss
                if 'MvpExp' in f and int(f['MvpExp'])>0: m['mvp']=1
                for key,mv in (('Drops',False),('MvpDrops',True)):
                    if key not in f: continue
                    body=f[key]
                    for name,val in re.findall(r'(\w+)\s*:\s*\(?\s*(\d+)',body):
                        if name not in aeg: continue
                        m.setdefault('_d',[]).append((aeg[name],int(val),mv))
                mobs[mid]=m
    for mid,m in mobs.items():
        for i,base,mv in m.pop('_d',[]):
            drops[i].append([mid,adj(base,i,m['boss'],mv),1 if mv else 0])
    # spawns + quests
    spawn=collections.defaultdict(lambda: collections.defaultdict(lambda:[0,0]))
    quests=collections.defaultdict(set)
    npc_main=os.path.join(root,'npc/re/scripts_main.conf')
    files=loaded_npc(root) if os.path.exists(npc_main) else []
    rx=re.compile(r'\b(getitem|getitem2|rentitem|getnameditem|getitembound|getitembound2|rentitem2)\b\s*\(?\s*("?)([A-Za-z_]\w*|\d+)\2\s*,')
    for f in files:
        t=open(os.path.join(root,f),encoding='latin-1').read()
        for m in re.finditer(r'^(\w[\w@-]*),\d+,\d+(?:,\d+,\d+)?\t(?:boss_)?monster\t[^\t]+\t(\d+),(\d+)(?:,(\d+))?',t,re.M):
            mp,mid,cnt,d1=m.group(1),int(m.group(2)),int(m.group(3)),int(m.group(4) or 0)
            s=spawn[mid][mp]; s[0]+=cnt; s[1]=max(s[1],d1)
        if '/mobs/' in f: continue
        body=re.sub(r'//.*','',t)
        for m in rx.finditer(body):
            v=m.group(3); i=int(v) if v.isdigit() else aeg.get(v)
            if i: quests[i].add(f)
    names={}
    if mnt not in ('-', '') and os.path.exists(mnt):
        for l in open(mnt,encoding='latin-1'):
            p=l.strip().split('#')
            if len(p)>=2 and p[0].endswith('.rsw'): names[p[0][:-4].lower()]=p[1]
    prev=None
    if not files and fallback and os.path.exists(fallback):
        prev=json.load(open(fallback,encoding='utf-8'))
    used=set(m for v in drops.values() for m,_,_ in v)
    M={}
    for mid in used:
        m=mobs[mid]; sp=sorted(spawn.get(mid,{}).items(),key=lambda x:-x[1][0])[:5]
        spawns=[[mp,names.get(mp.lower(),mp),c,d] for mp,(c,d) in sp]
        if not spawns and prev:
            old=(prev.get('mobs') or {}).get(str(mid)) or {}
            spawns=old.get('sp') or []
        M[mid]={'n':m.get('n','?'),'lv':m.get('lv',1),'b':1 if m['boss'] else 0,**({'mvp':1} if m.get('mvp') else {}),
                'sp':spawns}
    D={i:sorted(v,key=lambda x:-x[1]) for i,v in drops.items()}
    Q={str(i):sorted(v)[:6] for i,v in quests.items()}
    if not files and prev and isinstance(prev.get('quests'), dict):
        Q={str(i):list(v)[:6] for i,v in prev['quests'].items()}
    # String keys match parse_sources, which reads object keys as item/mob ids.
    out={'v':3,'rates':{k:bc[k] for k in sorted(bc) if k.startswith('item_')},'penalty':{str(k):v for k,v in penalty(root).items()},
         'mobs':{str(k):v for k,v in M.items()},'drops':{str(k):v for k,v in D.items()},'quests':Q}
    json.dump(out,open(outp,'w'),separators=(',',':'),ensure_ascii=False)
    print('npc files',len(files),'mobs',len(M),'spawning',sum(1 for m in M.values() if m['sp']),'items w/ drops',len(D),'quest items',len(Q))
if __name__=='__main__': main()
