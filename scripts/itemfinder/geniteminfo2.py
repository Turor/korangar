import re,sys,json
sys.path.insert(0,'.')
from lcfg import fields,list_entries
def items(path):
    t=open(path,encoding='utf-8',errors='replace').read()
    for k,ks,vs,ve in fields(t,0,len(t)):
        if t[vs]=='(':
            for s,e in list_entries(t,vs+1,ve-1):
                yield {k:t[a:b] for k,_,a,b in fields(t,s+1,e-1)}
            return
FIRST=['Swordsman','Magician','Archer','Acolyte','Merchant','Thief']
LINES={'Swordsman':['Knight','Crusader'],'Magician':['Wizard','Sage'],'Archer':['Hunter','Bard'],
       'Acolyte':['Priest','Monk'],'Merchant':['Blacksmith','Alchemist'],'Thief':['Assassin','Rogue'],
       'Taekwon':['Star_Gladiator','Soul_Linker']}
ORDER=['Novice','Swordsman','Knight','Crusader','Magician','Wizard','Sage','Archer','Hunter','Bard','Acolyte','Priest','Monk',
 'Merchant','Blacksmith','Alchemist','Thief','Assassin','Rogue','Taekwon','Star_Gladiator','Soul_Linker','Gunslinger','Rebellion',
 'Ninja','Kagerou','Summoner','Gangsi','Death_Knight','Dark_Collector']
LABEL={'Novice':'Novice/Super Novice','Bard':'Bard/Dancer','Kagerou':'Kagerou/Oboro','Star_Gladiator':'Star Gladiator',
 'Soul_Linker':'Soul Linker','Death_Knight':'Death Knight','Dark_Collector':'Dark Collector','Taekwon':'Taekwon'}
ALL=set(ORDER)-{'Gangsi','Death_Knight','Dark_Collector'}
def jobset(f):
    v=f.get('Job')
    if v is None: return set(ALL)
    kv={k:v[a:b] for k,_,a,b in fields(v,1,len(v)-1)}
    s=set(ALL) if kv.get('All')=='true' else set()
    for k,x in kv.items():
        if k=='All': continue
        if x=='true': s.add(k)
        else: s.discard(k)
    return s
def joinl(xs):
    if not xs: return "No job"
    return xs[0] if len(xs)==1 else ', '.join(xs[:-1])+' and '+xs[-1]
def render_jobs(s):
    core=s&ALL
    if core==ALL: return 'Every Job'
    if core==ALL-{'Novice'}: return 'Every Job except Novice'
    if ALL-core and len(ALL-core)<=3 and len(core)>12:
        return 'Every Job except '+joinl([LABEL.get(x,x.replace('_',' ')) for x in ORDER if x in ALL-core])
    out=[]; used=set()
    for b,sub in LINES.items():
        fam=[b]+sub
        if all(x in s for x in fam):
            out.append((ORDER.index(b),LABEL.get(b,b)+' classes')); used|=set(fam)
    for x in ORDER:
        if x in s and x not in used: out.append((ORDER.index(x),LABEL.get(x,x.replace('_',' '))))
    return joinl([t for _,t in sorted(out)])
def render_upper(v):
    if not v: return None
    fl=set(re.findall(r'ITEMUPPER_(\w+)',v))
    if 'ALL' in fl or fl>={'NORMAL','UPPER','BABY','THIRD','THIRDUPPER','THIRDBABY'}: return None
    parts=[]
    names=[('NORMAL','regular'),('UPPER','Transcendent'),('BABY','Baby'),('THIRD','3rd'),('THIRDUPPER','Transcendent 3rd'),('THIRDBABY','Baby 3rd')]
    if fl=={'THIRD','THIRDUPPER','THIRDBABY'}: return '3rd classes only'
    if fl=={'UPPER','THIRD','THIRDUPPER','THIRDBABY'}: return 'Transcendent and 3rd classes only'
    if fl=={'NORMAL','UPPER','BABY'}: return 'not 3rd classes'
    if fl=={'NORMAL'}: return 'regular (non-transcendent) classes only'
    if fl=={'UPPER'}: return 'Transcendent classes only'
    miss=[n for k,n in names if k not in fl]
    if len(miss)<=2: return 'except '+joinl(miss)+' classes'
    return joinl([n for k,n in names if k in fl])+' classes only'
def main():
    herc={}
    for f in items('re_item_db.conf'): herc[int(f['Id'])]=f
    clones={}
    for f in items('item_db2.conf'):
        i=int(f['Id'])
        if 'CloneItem' in f:
            base=dict(herc.get(int(f['CloneItem']),{})); base.update({k:v for k,v in f.items() if k!='CloneItem'}); base['_clone']=int(f['CloneItem']); herc[i]=base
        elif i in herc: herc[i].update(f)
        else: herc[i]=f
    src=open(sys.argv[1],encoding='latin-1').read().replace('\r\n','\n')
    blocks={int(m.group(1)):m.group(0).rstrip(',\n').rstrip(',') for m in re.finditer(r'^\t\[(\d+)\] = \{\n.*?\n\t\},?\n',src,re.M|re.S)}
    SEP='_______________________'
    REQ='^0000CCRequirement:^000000'
    def lua_lines(desc): return re.findall(r'"((?:[^"\\]|\\.)*)"',desc)
    changed=0; examples=[]
    out=['-- Turoran: RENEWAL item descriptions (llchrisll/ROenglishRE Translation/Renewal, kRO English).',
     '-- Requirement (Base Level / jobs / class tier / gender) and Weight lines are rendered from the live',
     '-- Hercules renewal item_db (+ item_db2), so Turoran equip changes show. slotCount = server Slots.',
     '-- MVP slot clones (+40000) copy their renewal base item. latin-1/CP949.','','tbl = {']
    for i in sorted(herc):
        f=herc[i]; base=f.get('_clone',i); b=blocks.get(base)
        if not b: continue
        b=re.sub(r'^\t\[\d+\]','\t[%d]'%i,b,count=1)
        b=re.sub(r'slotCount = \d+','slotCount = %d'%int(f.get('Slots','0') or 0),b,count=1)
        typ=f.get('Type','').strip('"')
        if typ in ('IT_WEAPON','IT_ARMOR'):
            m=re.search(r'(\bidentifiedDescriptionName = \{)(.*?)(\n\t\t\})',b,re.S)
            if m:
                L=lua_lines(m.group(2)); orig=list(L)
                # weight
                if 'Weight' in f:
                    w=int(f['Weight'])/10; ws=('%g'%w)
                    L=[re.sub(r'^(\^0000CCWeight:\^000000 )[\d.]+$',r'\g<1>'+ws,l) for l in L]
                req=[]
                el=f.get('EquipLv')
                if el:
                    n=re.findall(r'\d+',el)
                    if n and int(n[0])>1: req.append('Base Level %s'%n[0]+(' ~ %s'%n[1] if len(n)>1 else ''))
                jobs=render_jobs(jobset(f)); up=render_upper(f.get('Upper'))
                if jobs!='Every Job' or up:
                    req.append(jobs+(' ('+up+')' if up else ''))
                g=f.get('Gender','')
                if 'FEMALE' in g: req.append('Female only')
                elif 'MALE' in g: req.append('Male only')
                newreq=[REQ+' None'] if not req else [REQ]+req
                idx=next((k for k,l in enumerate(L) if l.startswith(REQ)),None)
                if idx is None:
                    L=L+[SEP]+newreq
                else:
                    j=idx+1
                    while j<len(L) and L[j]!=SEP and '^0000CC' not in L[j] and len(L[j])<140: j+=1
                    L=L[:idx]+newreq+L[j:]
                if L!=orig:
                    changed+=1
                    if len(examples)<400: examples.append((i,f.get('AegisName'),orig,L))
                    body='\n'+'\n'.join('\t\t\t"%s",'%l for l in L)
                    b=b[:m.start(2)]+body+b[m.end(2):]
        out.append(b+',')
    out.append('}')
    open(sys.argv[2],'w',encoding='latin-1',newline='\n').write('\n'.join(out)+'\n')
    json.dump(examples,open('equip_examples.json','w'))
    print('items written',len(out)-7,'equip text changed',changed)

if __name__=='__main__':
    main()
