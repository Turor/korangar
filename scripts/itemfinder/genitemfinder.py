"""Item Finder data: python3 genitemfinder.py <re/item_db.conf> <item_db2.conf> <itemInfo.lua> <out.json>
Weapons/armor/cards with stats and normalized script effects for the roBrowser Item Finder."""
import os,re,sys,json
sys.path.insert(0,__import__('os').path.dirname(__file__))
from geniteminfo2 import items,jobset,render_jobs,render_upper
WT={'W_DAGGER':'Dagger','W_1HSWORD':'One-Handed Sword','W_2HSWORD':'Two-Handed Sword','W_1HSPEAR':'One-Handed Spear','W_2HSPEAR':'Two-Handed Spear',
 'W_1HAXE':'One-Handed Axe','W_2HAXE':'Two-Handed Axe','W_MACE':'Mace','W_2HMACE':'Two-Handed Mace','W_STAFF':'Staff','W_2HSTAFF':'Two-Handed Staff',
 'W_BOW':'Bow','W_KNUCKLE':'Knuckle','W_MUSICAL':'Instrument','W_WHIP':'Whip','W_BOOK':'Book','W_KATAR':'Katar','W_REVOLVER':'Revolver',
 'W_RIFLE':'Rifle','W_GATLING':'Gatling Gun','W_SHOTGUN':'Shotgun','W_GRENADE':'Grenade Launcher','W_HUUMA':'Huuma Shuriken'}
LOC=[('EQP_HEAD_TOP','Upper Headgear'),('EQP_HEAD_MID','Mid Headgear'),('EQP_HEAD_LOW','Lower Headgear'),('EQP_ARMOR','Armor'),
 ('EQP_SHIELD','Shield'),('EQP_GARMENT','Garment'),('EQP_SHOES','Shoes'),('EQP_ACC','Accessory'),('EQP_WEAPON','Weapon'),('EQP_ARMS','Weapon'),('EQP_HAND_R','Weapon'),('EQP_HAND_L','Shield'),
 ('EQP_COSTUME','Costume'),('EQP_HELM','Headgear'),('EQP_ARMS','Weapon'),('EQP_SHADOW','Shadow')]
def locs(v):
    names=re.findall(r'EQP_\w+',v or ''); out=[]
    for n in names:
        if 'COSTUME' in n: lab='Costume'
        elif 'SHADOW' in n: lab='Shadow'
        else: lab=next((l for k,l in LOC if n.startswith(k)),n)
        if lab not in out: out.append(lab)
    return out
def nm(x):
    x=x.strip()
    for p,r in (('RC_',''),('Ele_',''),('Size_',''),('Class_',''),('RC2_','')):
        if x.startswith(p): return x[len(p):].replace('_',' ').title().replace('Demihuman','Demi-Human').replace('Demiplayer','Demi-Human Player')
    return x
STAT={'bStr':'STR','bAgi':'AGI','bVit':'VIT','bInt':'INT','bDex':'DEX','bLuk':'LUK','bAllStats':'All Stats'}
def num(x):
    try: return int(x.strip())
    except: return None
def eff(fn,a,cond):
    """-> (kind,target,value,text)"""
    v=num(a[-1]) if a else None
    t=[nm(x) for x in a[:-1]]
    T=lambda: ', '.join(t)
    pct=lambda k,txt: (k,T(),v,txt.format(t=T(),v=v if v is not None else a[-1].strip()))
    m={
     'bAddRace':('damage','{v}% physical damage vs {t}'),'bAddEle':('damage','{v}% physical damage vs {t} element'),'bAddSize':('damage','{v}% physical damage vs {t} size'),
     'bAddClass':('damage','{v}% physical damage vs {t} class'),'bAddRace2':('damage','{v}% physical damage vs {t}'),
     'bMagicAddRace':('damage','{v}% magical damage vs {t}'),'bMagicAddEle':('damage','{v}% magical damage vs {t} element'),'bMagicAddSize':('damage','{v}% magical damage vs {t} size'),
     'bMagicAddClass':('damage','{v}% magical damage vs {t} class'),'bMagicAtkEle':('damage','{v}% {t} magic damage'),'bSkillAtk':('damage','{v}% damage with {t}'),
     'bSubRace':('resist','{v}% resistance vs {t}'),'bSubEle':('resist','{v}% resistance vs {t} element'),'bSubSize':('resist','{v}% resistance vs {t} size'),
     'bSubClass':('resist','{v}% resistance vs {t} class'),'bSubRace2':('resist','{v}% resistance vs {t}'),'bMagicSubRace':('resist','{v}% magic resistance vs {t}'),
     'bSkillSubRate':('resist','{v}% resistance vs {t} skill'),'bSubDefEle':('resist','{v}% resistance vs {t}'),
    }
    if fn in m: return pct(*m[fn])
    if fn=='bCriticalAddRace': return ('crit',T(),v,'CRIT +{} vs {}'.format(v if v is not None else a[-1].strip(),T()))
    if fn in('bSkillVariableCast','bSkillFixedCast','bSkillCooldown','bSkillDelay'):
        lab={'bSkillVariableCast':'variable cast','bSkillFixedCast':'fixed cast','bSkillCooldown':'cooldown','bSkillDelay':'after-cast delay'}[fn]
        return ('cast',T().replace('_',' '),v,'{} {} {} ms'.format(T().replace('_',' '),lab,v if v is not None else a[-1].strip()))
    simple={
     'bAtkRate':('damage','ATK','ATK +{v}%'),'bMatkRate':('damage','MATK','MATK +{v}%'),'bCritAtkRate':('damage','Critical','Critical damage +{v}%'),
     'bLongAtkRate':('damage','Ranged','Ranged physical damage +{v}%'),'bShortAtkRate':('damage','Melee','Melee physical damage +{v}%'),
     'bNearAtkDef':('resist','Melee','Melee damage taken -{v}%'),'bLongAtkDef':('resist','Ranged','Ranged damage taken -{v}%'),
     'bMagicAtkDef':('resist','Magic','Magic damage taken -{v}%'),'bMiscAtkDef':('resist','Misc','Misc damage taken -{v}%'),
     'bDef':('def','DEF','DEF +{v}'),'bDefRate':('def','DEF%','DEF +{v}%'),'bDef2':('def','Soft DEF','Soft DEF +{v}'),
     'bMdef':('mdef','MDEF','MDEF +{v}'),'bMdefRate':('mdef','MDEF%','MDEF +{v}%'),'bMdef2':('mdef','Soft MDEF','Soft MDEF +{v}'),
     'bMaxHP':('hpsp','MaxHP','MaxHP +{v}'),'bMaxSP':('hpsp','MaxSP','MaxSP +{v}'),'bMaxHPrate':('hpsp','MaxHP%','MaxHP +{v}%'),'bMaxSPrate':('hpsp','MaxSP%','MaxSP +{v}%'),
     'bHPrecovRate':('hpsp','HP Regen','HP recovery +{v}%'),'bSPrecovRate':('hpsp','SP Regen','SP recovery +{v}%'),
     'bAspd':('aspd','ASPD','ASPD +{v}'),'bAspdRate':('aspd','ASPD%','ASPD +{v}%'),
     'bVariableCastrate':('cast','Variable Cast','Variable cast time {v}%'),'bFixedCastrate':('cast','Fixed Cast','Fixed cast time {v}%'),
     'bCastrate':('cast','Variable Cast','Cast time {v}%'),'bFixedCast':('cast','Fixed Cast (flat)','Fixed cast {v} ms'),'bVariableCast':('cast','Variable Cast (flat)','Variable cast {v} ms'),
     'bDelayrate':('cast','After-cast Delay','After-cast delay {v}%'),
     'bCritical':('crit','CRIT','CRIT +{v}'),'bCriticalLong':('crit','CRIT (ranged)','Ranged CRIT +{v}'),
     'bHit':('hitflee','HIT','HIT +{v}'),'bFlee':('hitflee','FLEE','FLEE +{v}'),'bFlee2':('hitflee','Perfect Dodge','Perfect Dodge +{v}'),'bHitRate':('hitflee','HIT%','HIT +{v}%'),'bFleeRate':('hitflee','FLEE%','FLEE +{v}%'),
     'bHealPower':('heal','Heal','Healing power +{v}%'),'bHealPower2':('heal','Received Heal','Received healing +{v}%'),'bAddItemHealRate':('heal','Item Heal','Item healing +{v}%'),
     'bBaseAtk':('damage','ATK (flat)','ATK +{v}'),'bAtk':('damage','ATK (flat)','ATK +{v}'),'bMatk':('damage','MATK (flat)','MATK +{v}'),
    }
    extra={'bNoMagicDamage':('resist','Magic','Nullifies {v}% magic damage'),'bUseSPrate':('other','SP Cost','SP cost {v}%'),
     'bIgnoreDefRace':('damage','Ignore DEF','Ignores DEF of {t}'),'bIgnoreMdefRace':('damage','Ignore MDEF','Ignores MDEF of {t}'),
     'bIgnoreDefRaceRate':('damage','Ignore DEF','Ignores {v}% DEF of {t}'),'bIgnoreMdefRaceRate':('damage','Ignore MDEF','Ignores {v}% MDEF of {t}'),
     'bResEff':('resist','Status','{v100}% resistance to {t}'),'bAddEff':('other','Inflict','{v100}% chance to inflict {t}'),
     'bSpeedRate':('other','Move Speed','Movement speed +{v}%'),'bUnbreakableArmor':('other','Unbreakable','Unbreakable armor'),
     'bUnbreakableWeapon':('other','Unbreakable','Unbreakable weapon'),'bHPDrainRate':('other','HP Drain','{v}% chance to drain HP'),
     'bSPDrainRate':('other','SP Drain','{v}% chance to drain SP'),'bHPGainValue':('hpsp','HP on kill','Gain {v} HP per kill'),'bSPGainValue':('hpsp','SP on kill','Gain {v} SP per kill'),
     'bPerfectHitRate':('hitflee','Perfect Hit','{v}% perfect hit'),'bSplashRange':('other','Splash','Splash attack')}
    if fn in extra:
        k,tg,txt=extra[fn]; tt=', '.join(x.replace('Eff ','').replace('Eff_','') for x in t)
        return (k,tg if fn!='bResEff' else tt,(v/100 if fn in('bResEff','bAddEff') and v is not None else v),txt.format(t=tt,v=v,v100=(v/100 if v is not None else '?')))
    if fn in STAT: return ('attr',STAT[fn],v,'{} +{}'.format(STAT[fn],v if v is not None else a[-1].strip()))
    if fn in simple:
        k,tg,txt=simple[fn]; return (k,tg,v,txt.format(v=v if v is not None else a[-1].strip()))
    if fn in ('bAutoSpell','bAutoSpellWhenHit','bAutoSpellOnSkill'):
        sk=a[0].strip().strip('"'); return ('autospell',sk,num(a[1]) if len(a)>1 else None,'Autocast {} Lv{}{}'.format(sk.replace('_',' '),a[1].strip() if len(a)>1 else '?',' when hit' if 'Hit' in fn else ''))
    return ('other',fn[1:],v,'{} {}'.format(fn[1:],', '.join(x.strip() for x in a)))
# --- effect magnitude ('m') -------------------------------------------------
# Every effect gets m = a number where BIGGER IS BETTER, used by the client to rank by matched effect.
#  * literal script value when there is one;
#  * otherwise the script expression evaluated at a reference character: refine +10, BaseLevel 175,
#    JobLevel 60, every readparam stat 100, skill levels 5 (flag 'me':1 = estimated);
#  * otherwise the largest number on the mapped description line (ranges "a~b" take b) ('me':1);
#  * kinds where lower is better (cast/delay %, SP cost) are negated, so "-10% cast" -> m=10.
REF={'getrefine()':'10','BaseLevel':'175','JobLevel':'60'}
def evalexpr(x):
    x=x.strip()
    if not x or '.@' in x or '?' in x or '"' in x: return None
    for k,r in REF.items(): x=x.replace(k,r)
    x=re.sub(r'readparam\(\s*b\w+\s*\)','100',x); x=re.sub(r'getskilllv\([^()]*\)','5',x)
    if re.search(r'[A-Za-z_]',re.sub(r'\b(min|max)\b','',x)): return None
    x=re.sub(r'(?<![/*])/(?![/*])','//',x)
    try: v=eval(x,{'__builtins__':{}},{'min':min,'max':max})
    except Exception: return None
    return v if isinstance(v,(int,float)) else None
LOWER_BETTER={'bSkillVariableCast','bSkillFixedCast','bSkillCooldown','bSkillDelay','bVariableCastrate','bFixedCastrate','bCastrate','bFixedCast','bVariableCast','bDelayrate','bUseSPrate'}
def desc_num(line):
    l=line.replace(',','')
    rg=re.findall(r'(\d+(?:\.\d+)?)\s*(?:~|to)\s*\+?(\d+(?:\.\d+)?)',l)
    if rg: return max(float(b) for _,b in rg)
    ns=[float(n) for n in re.findall(r'(?<![\w.])[+-]?(\d+(?:\.\d+)?)',l)]
    return max(ns) if ns else None
# v6 kinds: one unit per kind (flat vs %), one kind per stat
ATTRK={'STR':'str','AGI':'agi','VIT':'vit','INT':'int','DEX':'dex','LUK':'luk','All Stats':'allstats'}
FLATK={'bBaseAtk':'atk','bAtk':'atk','bMatk':'atk','bMaxHP':'hp','bMaxHPrate':'hp%','bMaxSP':'sp','bMaxSPrate':'sp%',
 'bHPrecovRate':'regen','bSPrecovRate':'regen','bHPGainValue':'onkill','bSPGainValue':'onkill',
 'bFixedCast':'castms','bVariableCast':'castms','bSkillVariableCast':'castms','bSkillFixedCast':'castms','bSkillCooldown':'castms','bSkillDelay':'castms',
 'bDelayrate':'delay','bAspdRate':'aspd%','bDefRate':'def%','bMdefRate':'mdef%','bHitRate':'hitflee%','bFleeRate':'hitflee%','bPerfectHitRate':'hitflee%'}
def rekind(fn,k,tg):
    if k=='attr': return ATTRK.get(tg,k)
    return FLATK.get(fn,k)
def parse_script(src):
    out=[]; depth=0; condstack=[]; pending_if=False; i=0
    strs=[]
    def _mask(m): strs.append(m.group(0)); return '"@%d@"'%(len(strs)-1)
    s=re.sub(r'"(?:[^"\\]|\\.)*"',_mask,src)
    unmask=lambda t: re.sub(r'"@(\d+)@"',lambda m: strs[int(m.group(1))],t)
    toks=re.finditer(r'\b(if|else|for|while)\b|[{};]|\b(bonus[2-5]?|skill|autobonus[23]?|bonus_script|itemskill|sc_start\d?)\b\s+([^;]*);',s)
    last_ctrl_end=-1
    for m in toks:
        g=m.group(0)
        if m.group(1): pending_if=True; continue
        if g=='{': condstack.append(pending_if); pending_if=False; continue
        if g=='}':
            if condstack: condstack.pop()
            continue
        if g==';': pending_if=False; continue
        kw,args=m.group(2),unmask(m.group(3))
        cond=pending_if or any(condstack); pending_if=False
        parts=[x for x in re.split(r',(?![^(]*\))',args)]
        if kw.startswith('bonus') and kw!='bonus_script':
            fn=parts[0].strip(); a=parts[1:]
            if any(re.search(r'[a-z]\(|getrefine|readparam|\*|/',x) for x in a): cond=True
            k,tg,v,txt=eff(fn,a,cond); k=rekind(fn,k,tg)
            mv,est=v,0
            if mv is None and a and fn not in('bAutoSpell','bAutoSpellWhenHit','bAutoSpellOnSkill'):
                mv=evalexpr(a[-1]); est=1
                if mv is not None and fn in('bResEff','bAddEff'): mv=mv/100
            if mv is not None and fn in LOWER_BETTER: mv=-mv
        elif kw=='skill':
            k,tg,v,txt='other','Skill',num(parts[1]) if len(parts)>1 else None,'Enables skill {} Lv{}'.format(parts[0].strip().replace('_',' '),parts[1].strip() if len(parts)>1 else '')
            mv,est=v,0
        elif kw.startswith('autobonus'):
            inner=re.findall(r'bonus[2-5]?\s+(b\w+)\s*,?([^;]*)',unmask(args))
            k,tg,v,txt='autospell','Autobonus',None,'Chance to gain (proc): '+', '.join((f[1:]+' '+x.strip())[:40] for f,x in inner[:4])
            cond=True; mv,est=None,0
        else:
            k,tg,v,txt='other',kw,None,(kw+' '+args.strip())[:80]; mv,est=None,0
        d={'k':k,'t':tg,'v':v,'x':txt,'c':1 if cond else 0}
        if mv is not None: d['m']=round(mv,2) if isinstance(mv,float) and not float(mv).is_integer() else int(mv)
        if est and mv is not None: d['me']=1
        out.append(d)
    return out
KW={'damage':['damage','attack','atk','matk'],'resist':['resist','reduc','taken','received','nullif'],'attr':['str','agi','vit','int','dex','luk','stat'],
 'def':['def'],'mdef':['mdef'],'hpsp':['hp','sp'],'aspd':['aspd','attack speed'],'cast':['cast','delay'],'crit':['critical','crit'],
 'hitflee':['hit','flee','dodge'],'heal':['heal','recover'],'autospell':['cast','chance','autocast','auto'],'other':[]}
CC=re.compile(r'\^[0-9a-fA-F]{6}')
INFO=re.compile(r'^(Class|Type|Weight|Requirement|Jobs?|Compound on|Location|Attack|Defense|Weapon Level|Armor Level|Level Requirement|Base Level|Position)\s*:',re.I)
def desc_lines(raw):
    """-> list of (text, header_of_group) for effect-ish lines; stops at the stats/requirement footer"""
    out=[]; groups=[[]]
    for l in raw:
        l=re.sub(r'<NAVI>(.*?)<INFO>.*?</INFO></NAVI>',r'\1',l,flags=re.I)
        l=CC.sub('',l).replace('\\"','"').strip()
        if re.match(r'^_{3,}',l): groups.append([]); continue
        if INFO.match(l): break
        if l: groups[-1].append(l)
    for gi,g in enumerate(groups):
        if not g: continue
        if gi==0 and len(groups)>1 and not any(re.search(r'\d|%',x) for x in g): continue   # flavor block
        hdr=None
        for l in g:
            if l.endswith(':'): hdr=l; out.append((l,None)); continue
            out.append((l,hdr))
    return out
def words(t):
    t=str(t).lower().replace('-',' ')
    return [w for w in re.split(r'[ ,/]+',t) if len(w)>2 and w not in('the','all','flat')]
def map_desc(lines,effs):
    texts=[l for l,_ in lines]
    assign=[[] for _ in lines]; low=[l.lower().replace('-',' ') for l in texts]
    for ei,e in enumerate(effs):
        best,bi=0,-1
        for li,l in enumerate(low):
            if lines[li][1] is None and texts[li].endswith(':'): continue
            sc=0
            v=e['v']
            if isinstance(v,(int,float)):
                vs=str(abs(int(v))) if float(v).is_integer() else str(abs(v))
                if re.search(r'(?<![\d.])%s(?![\d])'%re.escape(vs),l): sc+=2
            tw=words(e['t'])
            if tw and any(w in l for w in tw): sc+=2
            if any(re.search(r'\b'+re.escape(k),l) for k in KW.get(e['k'],[])): sc+=1
            if sc>best: best,bi=sc,li
        if best>=2: assign[bi].append(ei)
    out=[]
    for li,(l,hdr) in enumerate(lines):
        if hdr is None and l.endswith(':'):
            out.append({'x':l,'b':[],'h':1,'c':1}); continue
        ix=assign[li]
        cond=1 if hdr or (ix and all(effs[i]['c'] for i in ix)) else 0
        out.append({'x':l,'b':ix,'c':cond})
    # drop headers with nothing under them
    res=[]
    for i,d in enumerate(out):
        if d.get('h') and (i+1>=len(out) or out[i+1].get('h')): continue
        res.append(d)
    return res
# GM-only items are hidden from the Finder. Rule: name/description mentions GM / Game Master / GM-only,
# or the item has an absurd permanent effect no real item has (DEF >= 1000, weapon ATK >= 5000,
# All Stats >= 50, >= 100% cast reduction, >= 5000 ms flat cast/cooldown reduction).
GMTXT=re.compile(r'\bGM\b|game ?master|gm[- ]only',re.I)
def gm_reason(e,desc):
    if GMTXT.search(e['n']) or GMTXT.search(' '.join(desc)): return 'GM text'
    if e.get('def',0)>=1000: return 'DEF %d'%e['def']
    if e.get('atk',0)>=5000: return 'ATK %d'%e['atk']
    for x in e['eff']:
        m=x.get('m')
        if x['c'] or not isinstance(m,(int,float)): continue
        if x['k']=='allstats' and m>=50: return 'All Stats +%s'%m
        if x['k']=='cast' and m>=100: return 'cast -%s%%'%m
        if x['k']=='castms' and m>=5000: return 'cast/cooldown -%s ms'%m
    return None
def main():
    db,db2,info,outp=sys.argv[1:5]
    herc={}
    for f in items(db): herc[int(f['Id'])]=f
    # item_db2 is optional. Pass "-" or a missing path when the Hercules tree has no import DB.
    if db2 not in ('-', '') and os.path.exists(db2):
        for f in items(db2):
            i=int(f['Id'])
            if 'CloneItem' in f:
                b=dict(herc.get(int(f['CloneItem']),{})); b.update({k:v for k,v in f.items() if k!='CloneItem'}); b['_clone']=int(f['CloneItem']); herc[i]=b
            elif i in herc: herc[i].update(f)
            else: herc[i]=f
    names={}; descs={}
    txt=open(info,encoding='latin-1').read()
    for m in re.finditer(r'^\t\[(\d+)\] = \{(.*?)^\t\},?$',txt,re.M|re.S):
        b=m.group(2); i=int(m.group(1))
        n=re.search(r'(?<!un)identifiedDisplayName = "((?:[^"\\]|\\.)*)"',b)
        if n: names[i]=n.group(1)
        d=re.search(r'(?<!un)identifiedDescriptionName = \{(.*?)\n\t\t\}',b,re.S)
        if d: descs[i]=[x.replace('\\"','"') for x in re.findall(r'"((?:[^"\\]|\\.)*)"',d.group(1))]
    out=[]; excluded=[]
    for i,f in sorted(herc.items()):
        typ=f.get('Type','').strip('"')
        cat={'IT_WEAPON':'weapon','IT_ARMOR':'armor','IT_CARD':'card'}.get(typ)
        # Enchant split: card-type items whose AegisName is not a monster card (*_Card)
        if cat=='card' and not f.get('AegisName','').strip('"').lower().endswith('_card'): cat='enchant'
        if not cat: continue
        sc=f.get('Script','')
        sc=sc[2:-2] if sc.startswith('<"') else sc
        n=names.get(i) or f.get('Name','').strip('"')
        L=locs(f.get('Loc'))
        if cat=='armor' and ('Costume' in L or 'Shadow' in L): sub=L[0]
        e={'id':i,'n':n,'c':cat,'s':int(f.get('Slots',0) or 0),'w':int(f.get('Weight',0) or 0)/10,
           'lv':int((re.findall(r'\d+',f.get('EquipLv','0')) or ['0'])[0]),
           'j':render_jobs(jobset(f)),'loc':L,'eff':parse_script(sc)}
        dl=desc_lines(descs.get(i,[]))
        if dl:
            e['d']=map_desc(dl,e['eff'])
            if not any(not x.get('h') for x in e['d']): del e['d']
            else:
                for ln in e['d']:
                    for ix in ln['b']:
                        ef=e['eff'][ix]
                        if 'm' not in ef and ef['k'] not in('autospell','other'):
                            n=desc_num(ln['x'])
                            if n is not None:
                                ef['m']=int(n) if float(n).is_integer() else n; ef['me']=1
                                if ef['k']=='cast': ef['m']=abs(ef['m'])
        up=render_upper(f.get('Upper'))
        if up: e['j']+=' ('+up+')'
        if cat=='weapon':
            e['wt']=WT.get(f.get('Subtype','').strip('"'),f.get('Subtype','').strip('"'))
            e['atk']=int(f.get('Atk',0) or 0); e['matk']=int(f.get('Matk',0) or 0); e['wl']=int(f.get('WeaponLv',0) or 0)
        if cat=='armor': e['def']=int(f.get('Def',0) or 0)
        if cat not in('card','enchant'): e['r']=0 if f.get('Refine','true')=='false' else 1
        if '_clone' in f:
            if i-f['_clone']==40000: e['mvp']=f['_clone']   # slotted MVP version
            else: e['base']=f['_clone']
        why=gm_reason(e,descs.get(i,[]))
        if why: excluded.append((i,e['n'],why)); continue
        out.append(e)
    json.dump({'v':7,'items':out},open(outp,'w'),separators=(',',':'),ensure_ascii=False)
    import collections
    print('excluded GM items:',len(excluded)); [print('  ',*x) for x in excluded]
    print(len(out),collections.Counter(x['c'] for x in out),sum('d' in x for x in out),collections.Counter(k['k'] for x in out for k in x['eff']).most_common())
if __name__=='__main__': main()
