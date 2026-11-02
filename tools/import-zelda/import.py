#!/usr/bin/env python3
"""Rebuild Wizard's Zelda adaptation from pinned scripts and documented authoring rules."""
import copy, hashlib, json, math, re
from pathlib import Path
ROOT = Path(__file__).resolve().parents[2]
SOURCES = ROOT / 'docs/imports/sources/wizard'
source = json.loads((SOURCES / 'scripts.json').read_text())
scripts = {article: {s['Name']: s['Data'] for s in ss} for article, ss in source['scripts'].items()}

def expand(s):
    while (m := re.search(r'for\((\d+) Iterations\)\{', s)):
        pos=m.end(); depth=1
        while depth:
            depth += (s[pos]=='{')-(s[pos]=='}');pos+=1
        s=s[:m.start()]+expand(s[m.end():pos-1])*int(m[1])+s[pos:]
    return s

def args(s):
    return {k:v.strip() for k,v in re.findall(r'(\w+)=(.*?)(?=, \w+=|$)',s)}

def num(a,k,default=0):
    try:return float(a.get(k,default))
    except ValueError:return default

def attacks(article,name):
    t=0; live={}; out=[]
    def clear(i):
        if i in live:
            b=live.pop(i);b['end']=t-1
            if b['end']>=b['start']:out.append(b)
    for line in expand(scripts[article][name]).splitlines():
        if m:=re.search(r'frame\(Frame=(\d+)\)',line):t=int(m[1])
        if m:=re.search(r'wait\(Frames=(\d+)\)',line):t+=int(m[1])
        if 'AttackModule::clear_all()' in line:
            for i in list(live):clear(i)
        elif m:=re.search(r'AttackModule::clear\(ID=(\d+)\)',line):clear(int(m[1]))
        if m:=re.search(r'\bATTACK\((.*)\)',line):
            a=args(m[1]);i=int(a['ID']);clear(i);live[i]={'start':t,'end':t,'source':a}
    for i in list(live):clear(i)
    return sorted(out,key=lambda b:(b['start'],int(b['source']['ID'])))

ledger={}
def hit(a,start=0,end=None,x=None,y=None,r=None):
    angle=num(a,'Angle');kind='normal'
    if angle==361:kind='sakurai';angle=45
    elif angle in (366,367,368):kind='autolink';angle=90
    elif 180<angle<360: angle-=360;kind='spike' if angle<=-80 else 'normal'
    b={'start':start,'x':num(a,'Z')*5 if x is None else x,'y':num(a,'Y')*5 if y is None else y,'r':num(a,'Size')*5 if r is None else r,'damage':num(a,'Damage'),'bkb':num(a,'BKB'),'kbg':num(a,'KBG')/100,'angle':angle}
    if end is not None:b['end']=end
    if kind!='normal':b['type']=kind
    if a.get('FacingRestrict','').endswith('_POS'):b['dir']='away'
    if a.get('FacingRestrict','').endswith('_B'):b['dir']='back'
    mask=a.get('Ground_or_Air','')
    if mask.endswith('_G'):b['situation']='ground'
    elif mask.endswith('_A'):b['situation']='air'
    if num(a,'FKB'):
        # Source WBKB resolved at 100 reference weight, then retained as fixed-launch adaptation.
        b['setKb']=True;b['bkb']=((1+num(a,'FKB')/2)*1.4+18)*num(a,'KBG')/100+num(a,'BKB')
    if num(a,'Rehit'):b['rehit']=int(num(a,'Rehit'))
    if num(a,'Hitlag',1)!=1:b['hitlagMult']=num(a,'Hitlag',1)
    if num(a,'ShieldDamage') and b['damage']>0:b['shieldDmgMult']=max(0,(b['damage']+num(a,'ShieldDamage'))/b['damage'])
    return b

def geom(slot,a,t):
    bone=a.get('Bone','');i=int(a['ID'])
    if 'top' in bone:return num(a,'Z')*5,num(a,'Y')*5
    if slot=='ftilt':return 18+i*11,56+i*.5
    if slot=='utilt':
        ang=math.radians(20+(t-7)/12*140);radius=44 if i==0 else 26 if i==1 else 10
        return math.cos(ang)*radius,64+math.sin(ang)*radius
    if slot in ('fair','bair'):
        s=1 if slot=='fair' else -1
        return ({1:26,2:0,3:58}.get(i,58)*s,{1:25,2:35,3:22}.get(i,22))
    if slot=='dair':return (2, -8) if 'foot' in bone else (0,18)
    if 'head' in bone:return 0,66
    return 0,40

def boxes(slot,article,name):
    result=[];raw=attacks(article,name);ledger[slot]={'article':article,'script':name,'attacks':raw}
    for v in raw:
        a=v['source'];start,end=v['start'],v['end']
        if slot=='fair':start-=3;end-=3
        if slot=='nspec':start-=2;end-=2
        x,y=geom(slot,a,start);b=hit(a,start,end,x,y)
        if slot=='utilt':b['frames']=[[t,*geom(slot,a,t),b['r']] for t in range(start,end+1)]
        # Root capsules are projected onto the gameplay plane and sampled at <= radius spacing.
        if a.get('X2','LUA_VOID')!='LUA_VOID' and 'top' in a.get('Bone',''):
            x2,y2=num(a,'Z2')*5,num(a,'Y2')*5
            steps=max(1,math.ceil(math.hypot(x2-x,y2-y)/max(1,b['r'])))
            for i in range(steps+1):
                c=copy.deepcopy(b);c['x']=x+(x2-x)*i/steps;c['y']=y+(y2-y)*i/steps
                if not result or c!=result[-1]:result.append(c)
        else:result.append(b)
    return result

def pose(**kw):return kw
stand={'head':[0,65],'elbowF':[15,47],'handF':[23,51],'elbowB':[-15,45],'handB':[-24,42]}
cast={'head':[3,64],'elbowF':[23,51],'handF':[49,49],'elbowB':[-12,52],'handB':[-20,63]}
up={'head':[0,65],'elbowF':[18,80],'handF':[5,105],'elbowB':[-18,75],'handB':[-5,95]}
kick={'head':[-21,55],'shoulder':[-12,44],'hip':[4,35],'kneeF':[29,24],'footF':[58,22],'kneeB':[-14,16],'footB':[-25,5],'handF':[8,51],'handB':[-29,56]}
backkick={'head':[22,55],'hip':[5,33],'kneeB':[-30,25],'footB':[-55,22],'kneeF':[20,18],'footF':[28,4],'handF':[37,52]}
low={'head':[6,45],'hip':[-4,18],'kneeF':[24,10],'footF':[43,7],'kneeB':[-20,9],'footB':[-30,0],'handF':[16,41]}
down={'head':[0,67],'kneeF':[2,17],'footF':[2,-8],'kneeB':[-15,18],'footB':[-21,8],'handF':[22,70],'handB':[-22,70]}
poses={'stand':stand,'cast':cast,'conjure':up,'lightning':kick,'backlightning':backkick,'lowMagic':low,'meteor':down}

def anim(p,start,end,total,air=False):
    return [{'at':0,'pose':'air' if air else 'stand'},{'at':max(0,start-3),'pose':p,'lerp':True},{'at':end,'pose':p},{'at':min(total-1,end+12),'pose':'air' if air else 'stand','lerp':True}]

def phase(name,frames,bs,**kw):return dict(name=name,frames=frames,hitboxes=bs,**kw)

def cut(bs,lo,hi):
    out=[]
    for b in bs:
        if b['start']>hi or b.get('end',hi)<lo:continue
        v=copy.deepcopy(b);v['start']=max(lo,b['start'])-lo;v['end']=min(hi,b.get('end',hi))-lo
        if 'frames' in v:v['frames']=[[t-lo,x,y,r] for t,x,y,r in v['frames'] if lo<=t<=hi]
        out.append(v)
    return out

def landing(lag):return {'on':'land','to':'landing','lag':lag}
def timeout(to):return {'on':'timeout','to':to}

def normal(slot,name,script,total,p,charge=None,air=None):
    bs=boxes(slot,'zelda',script);start=min(b['start'] for b in bs);end=max(b['end'] for b in bs)
    ph=phase('main',total,bs,anim=anim(p,start,end,total,bool(air)),events=[{'at':start,'sfx':'smash' if charge is not None else 'slash'}])
    if charge is not None:ph['charge']={'frame':charge}
    move={'name':name,'phases':[ph]}
    if air:
        lag,acstart,acend=air
        boundaries=[0,acstart,acend,total];parts=[]
        for lo,hi in zip(boundaries,boundaries[1:]):
            if lo==hi:continue
            pn='initial' if lo==0 else 'active' if lo==acstart else 'late'
            tr=landing(lag) if lo==acstart else {'on':'land','to':'end'}
            parts.append(phase(pn,hi-lo,cut(bs,lo,hi-1),anim=([{'at':0,'pose':'air'}] if lo==0 else [{'at':0,'pose':p},{'at':max(1,end-lo+8),'pose':'air','lerp':True}]),transitions=[tr]))
        for i in range(len(parts)-1):parts[i]['transitions'].append(timeout(parts[i+1]['name']))
        move.update(air='aerial',phases=parts)
    return move
moves={}
for spec in [('jab','Arcane Flurry','Attack11',24,'cast'),('ftilt','Crystal Edge','AttackS3',36,'cast'),('utilt','Ascending Arc','AttackHi3',29,'conjure'),('dtilt','Low Spark','AttackLw3',21,'lowMagic'),('dashatk','Palm Burst','AttackDash',35,'cast'),('fsmash','Binding Blast','AttackS4',49,'cast',10),('usmash','Radiant Storm','AttackHi4',63,'conjure',3),('dsmash','Twin Sweeps','AttackLw4',37,'lowMagic',2)]:
    slot,name,script,total,p,*charge=spec;moves[slot]=normal(slot,name,script,total,p,charge[0] if charge else None)
moves['utilt']['phases'][0]['anim']=[{'at':0,'pose':'stand'}]+[{'at':t,'pose':{'handF':list(geom('utilt',{'Bone':'armr','ID':'0'},t)),'elbowF':[geom('utilt',{'Bone':'armr','ID':'0'},t)[0]*.5,60]},'lerp':True} for t in [7,10,13,16,19]]+[{'at':28,'pose':'stand','lerp':True}]
# One rehit clock spans each sustained multi-hit cluster; fresh windows share the move's hit registry.
for b in moves['usmash']['phases'][0]['hitboxes']:
    if b['start']<34:b['rehit']=4
for slot, rate in [('jab',3),('fsmash',2),('usmash',4),('dsmash',8)]:
    for b in moves[slot]['phases'][0]['hitboxes']: b['rehit']=rate
# Jab enters a 2-tick rapid loop while A is held; releasing plays the separate finisher.
moves['jab']['phases'][0]['transitions']=[{'on':'attackHeld','from':9,'to':'rapidStart'}]
rapid=boxes('rapid','zelda','Attack100Sub')
for b in rapid:b['start']=0;b['end']=0;b['rehit']=2;b['hitstunAdd']=3;b['shieldStunMult']=8
fin=boxes('jabFinish','zelda','Attack100End')
moves['jab']['phases'] += [phase('rapidStart',4,[],anim=[{'at':0,'pose':'cast'}],transitions=[timeout('rapid')]),phase('rapid',2,rapid,anim=[{'at':0,'pose':'cast'}],transitions=[{'on':'release','to':'finish'},timeout('rapid')]),phase('finish',42,fin,anim=anim('cast',6,7,42))]
for spec in [('nair','Orbiting Lights','AttackAirN',50,'conjure',(12,4,38)),('fair','Lightning Kick','AttackAirF',49,'lightning',(15,3,43)),('bair','Reverse Lightning','AttackAirB',49,'backlightning',(16,3,45)),('uair','Skyfire','AttackAirHi',54,'conjure',(12,4,54)),('dair','Meteor Heel','AttackAirLw',44,'meteor',(12,4,40))]:
    slot,name,script,total,p,a=spec;moves[slot]=normal(slot,name,script,total,p,air=a)
    if slot=='nair':
        for ph in moves[slot]['phases']:
            for b in ph['hitboxes']:b['rehit']=4
for slot,script,start,total in [('grab','Catch',10,39),('dashgrab','CatchDash',13,47)]:
    # Telekinetic grab capsules use script root offsets.
    s=scripts['zelda'][script];a=args(re.search(r'CATCH\((.*)\)',s)[1]);a.update(Damage='0',BKB='0',KBG='0',Angle='0',FacingRestrict='F',Size=a.get('Size','3.8'))
    bs=[]
    for x in [num(a,'Z'),num(a,'Z2')]:
        b=hit(a,start,start+1,x*5,num(a,'Y')*5);b['type']='grab';bs.append(b)
    moves[slot]={'name':'Dash Bind' if slot=='dashgrab' else 'Telekinetic Bind','phases':[phase('main',total,bs,anim=anim('cast',start,start+1,total))]}
ns=boxes('nspec','zelda','SpecialN')
for b in ns:b['rehit']=4
moves['nspec']={'name':"Nayru's Love",'bReverse':True,'phases':[phase('main',57,ns,reflector={'from':4,'to':40,'speed':1.8,'damage':1.25,'radius':65,'height':40},events=[{'at':4,'invuln':10,'sfx':'counter'},{'at':11,'sfx':'slash'},{'at':26,'sfx':'slash'}],anim=anim('conjure',4,40,57),attach={'ward':{'show':[4,40],'angle':[[4,0],[40,720]]}})]}
# Farore: aim once on disappearance, travel for 17 ticks, then branch by landing state.
first=boxes('uspecStart','zelda','SpecialHiStart');airhi=boxes('uspecAir','zelda','SpecialAirHi');groundhi=boxes('uspecGround','zelda','SpecialHi')
for bs in (airhi,groundhi):
    for b in bs:b['start']=1;b['end']=2;b['x']=0;b['y']=36
moves['uspec']={'name':"Farore's Wind",'air':'upSpecial','phases':[
 phase('start',17,first,airAccelMult=0,events=[{'at':0,'setVel':{'x':0,'y':0},'sfx':'charge'}],anim=[{'at':0,'pose':'conjure'}],transitions=[timeout('travel')]),
 phase('travel',17,[],travelSpeed=21,hidden=True,ledgeGrab=0,events=[{'at':0,'leaveGround':True,'invuln':18,'sfx':'roll'}],transitions=[{'on':'timeout','if':'grounded','to':'groundEnd'},{'on':'timeout','to':'airEnd'}]),
 phase('airEnd',41,airhi,airAccelMult=.3,events=[{'at':0,'setVel':{'x':0,'y':0},'sfx':'smash'}],anim=[{'at':0,'pose':'conjure'}],ledgeGrab=3,transitions=[landing(30),timeout('helpless')]),
 phase('groundEnd',41,groundhi,events=[{'at':0,'setVel':{'x':0,'y':0},'sfx':'smash'}],anim=[{'at':0,'pose':'cast'}],transitions=[timeout('end')]) ]}
# Controlled articles use a release phase on the fighter, independent of their own lifetime.
moves['sspec']={'name':"Din's Fire",'bReverse':True,'phases':[phase('guide',56,[],events=[{'at':14,'spawn':'fire','sfx':'charge'}],anim=[{'at':0,'pose':'cast'}],transitions=[{'on':'articleRelease','to':'release'},timeout('release')]),phase('release',39,[],anim=anim('cast',0,12,39))]}
moves['dspec']={'name':'Phantom Slash','phases':[phase('build',66,[],events=[{'at':0,'spawn':'phantom','sfx':'charge'}],anim=[{'at':0,'pose':'conjure'}],transitions=[{'on':'articleRelease','to':'release'},timeout('end')]),phase('release',39,[],anim=anim('cast',0,12,39))]}

def line(a,b,w,c):return {'shape':'line','from':a,'to':b,'width':w,'color':c}
def circle(at,r,color,alpha=1):return {'shape':'circle','at':at,'r':r,'fill':color,'alpha':alpha}
def poly(points,c):return {'shape':'poly','points':points,'fill':c}
phantom_shapes=[poly([[-13,18],[-18,58],[0,77],[22,59],[15,20]],'#58546c'),circle([0,83],13,'#a5a3bf'),line([-12,24],[-26,0],10,'#76758d'),line([12,24],[27,0],10,'#76758d'),poly([[-25,55],[-8,64],[2,41],[-21,27]],'#7bbee3'),line([14,56],[55,103],8,'#d9d7ed'),line([14,56],[48,95],3,'#a397ef'),circle([0,58],8,'#dba9f3')]
stages=[]
for threshold,script,damage,hp,start,duration,speed in [(0,'AttackKick',5.9,8,10,16,3.5),(20,'AttackPunch',8.2,10,7,16,5),(28,'AttackS',10.5,13,7,16,7),(33,'AttackS',11.8,15,7,16,7),(38,'AttackL',14.1,18,7,16,9),(45,'AttackL',15.4,20,7,16,9),(50,'AttackMax',17.7,25,6,22,12)]:
    bs=boxes('phantom'+str(threshold),'zelda_phantom',script)
    sword_index=0
    for b in bs:
        if b['damage']==0:
            b['type']='windbox';b['bkb']=.5;b.pop('setKb',None)
        else:
            b['damage']=damage
            if script in ('AttackS','AttackL','AttackMax'):
                # Hand/sword coordinates fitted from the corresponding swing visuals.
                a=[v['source'] for v in ledger['phantom'+str(threshold)]['attacks'] if num(v['source'],'Damage')>0][sword_index];sword_index+=1
                if 'handr' in a['Bone']:
                    distance=num(a,'Z')*5
                    def blade_angle(t):
                        if script=='AttackMax':return -40+(t-6)*14
                        if script=='AttackL':return 105-(t-7)*45
                        return 20-(t-7)*15
                    b['frames']=[[t,12+distance*math.cos(math.radians(blade_angle(t))),50+distance*math.sin(math.radians(blade_angle(t))),b['r']] for t in range(b['start'],b['end']+1)]
                    b['x'],b['y']=b['frames'][0][1:3]
        if b['start']>=start:b['end']=min(b['end'],duration-1)
    extra={'AttackKick':1,'AttackPunch':5,'AttackS':3,'AttackL':1,'AttackMax':3}[script]
    for b in bs:
        b['start']+=extra;b['end']+=extra
        for key in b.get('frames',[]):key[0]+=extra
    start+=extra;duration+=extra
    stages.append({'from':threshold,'startup':start,'frames':duration,'vel':[speed,0],'health':hp,'hitboxes':bs,'shapes':([phantom_shapes[j] for j in [2,3]] if threshold==0 else phantom_shapes[:4] if threshold==20 else phantom_shapes[:5] if threshold<50 else phantom_shapes),'attackShapes':[v for j,v in enumerate(phantom_shapes) if j not in (5,6)]})
for stage in stages:
    if any(b.get('frames') for b in stage['hitboxes']):stage['stroke']={'width':7,'color':'#d9d7ed'}
stages[-1]['autoStartup']=6
# Din's center and outer blast scale continuously with travel age, independently of the fighter's charge.
firestages=[]
for age in range(43):
    frac=max(0,min(1,(age-16)/26));damage=7+7*frac;outer=3.5+3.5*frac
    center={'start':14,'end':19,'x':0,'y':0,'r':17.5+12.5*frac,'damage':damage,'bkb':50,'kbg':.97,'angle':52}
    sour={**center,'r':24+21*frac,'damage':outer,'bkb':55,'kbg':.4,'angle':65}
    firestages.append({'from':age,'startup':14,'frames':20,'vel':[0,0],'hitboxes':[center,sour],'shapes':[circle([0,0],10+6*frac,'#ffab58'),circle([0,0],5,'#fff4b0')],'attackShapes':[circle([0,0],sour['r'],'#ff7c43',.35),circle([0,0],center['r'],'#ffd68a',.6),{'shape':'star','at':[0,0],'r':center['r'],'fill':'#fff1d7'}]})
projectiles={
 'fire':{'name':"Din's Fire",'spawn':[25,45],'vel':[8,0],'r':12,'life':100,'max':1,'hit':{**firestages[0]['hitboxes'][0]},'article':{'slot':'sspec','minRelease':16,'autoRelease':42,'releaseOnUp':True,'steer':2.4,'stages':firestages}},
 'phantom':{'name':'Phantom Slash','spawn':[-48,0],'vel':[0,0],'r':24,'life':160,'max':1,'hit':{**stages[-1]['hitboxes'][-1]},'article':{'slot':'dspec','minRelease':15,'autoRelease':121,'hurtbox':{'x1':-22,'x2':22,'y1':0,'y2':98},'stages':stages}}
}
throws={}
for key,script,release,total in [('f','ThrowF',30,49),('b','ThrowB',27,49),('u','ThrowHi',30,49),('d','ThrowLw',42,61)]:
    raw=scripts['zelda'][script];a=args(re.search(r'ATTACK_ABS\((Kind=FIGHTER_ATTACK_ABSOLUTE_KIND_THROW,.*?)\)\r?\n',raw)[1]);a.update(Size='1',X='0',Y='0',Z='0')
    throws[key]={'release':release,'total':total,'hit':hit(a)}
    if key=='d':throws[key]['ticks']=[{'at':t,'damage':1.5} for t in [25,30,35,39]]
attachments=[
 {'id':'hat','joint':'head','shapes':[poly([[-12,4],[12,4],[7,12],[-8,12]],'playerDark'),line([-13,6],[13,6],3,'#e9d48a'),circle([0,8],3,'#cfb4ff')]},
 {'id':'mantle','joint':'shoulder','behind':True,'shapes':[poly([[-12,3],[11,3],[22,-45],[-25,-45]],'playerDark'),line([-14,-4],[14,-4],3,'#e9d48a')]},
 {'id':'ward','joint':'hip','hidden':True,'shapes':[{'shape':'poly','points':[[-58,0],[0,58],[58,0],[0,-58]],'fill':'#8adfff','alpha':.18,'stroke':'#c6edff','width':2}]},
 {'id':'spark','joint':'footF','hidden':True,'shapes':[{'shape':'star','at':[0,0],'r':15,'fill':'#ffffff'},circle([0,0],21,'#c69cff',.25)]}
]
for id,joint in [('backSpark','footB'),('skySpark','handF')]:
    a=copy.deepcopy(attachments[-1]);a['id']=id;a['joint']=joint
    if id=='skySpark':
        for shape in a['shapes']:shape['at']=[-2.5,27.5]
    attachments.append(a)
# Align magic/kick tells with the authored hit regions instead of retaining a staff.
for slot in ['fair','bair','uair','dair']:
    for ph in moves[slot]['phases']:
        if ph['hitboxes']:
            ph['attach']={('backSpark' if slot=='bair' else 'skySpark' if slot=='uair' else 'spark'):{'show':[min(b['start'] for b in ph['hitboxes']),max(b['end'] for b in ph['hitboxes'])]}}
fighter={'$schema':'../../schema/fighter.schema.json','schemaVersion':1,'extends':'base','name':'Wizard','tagline':'A spark, a knight, a perfect read','blurb':'A Zelda-inspired caster: precise lightning kicks, multi-hit magic, a guiding fireball, a directional warp and a delayed Phantom.','stats':{'weight':100/85,'walkSpeed':.914*3.5,'runSpeed':1.43*3.5,'dashSpeed':1.958*3.5,'dashFrames':16,'runAccel':.28,'airSpeed':1.092*3.5,'airAccel':.065*3.5,'gravity':.071*6,'fallSpeed':1.35*6,'fastFallSpeed':2.16*6,'jumpSpeed':11.75,'shortHopSpeed':8.6,'airJumpSpeed':12,'airJumpDrift':3.822},'ai':{'reach':70,'counterChance':0,'specials':[{'minDistance':160,'maxDistance':470,'height':100,'chance':.24,'grounded':True,'noProjectile':True,'down':True,'hold':None},{'minDistance':150,'maxDistance':550,'height':100,'chance':.15,'grounded':True,'noProjectile':True,'down':False,'hold':[18,40]}]},'poses':poses,'moves':moves,'throws':throws,'projectiles':projectiles,'attachments':attachments}
(ROOT/'assets/fighters/wizard/fighter.json').write_text(json.dumps(fighter,indent=2)+'\n')
(SOURCES/'ledger.json').write_text(json.dumps({'sourceCommit':source['commit'],'geometryScale':5,'rawAttackLedger':ledger,'ufd':{'url':'https://ultimateframedata.com/zelda','accessed':'2026-11-02','timings':{slot:{'phases':[(p['name'],p['frames']) for p in m['phases']]} for slot,m in moves.items()}}},indent=2)+'\n')
print('Rebuilt Wizard; run just bless to format content and update schemas.')
