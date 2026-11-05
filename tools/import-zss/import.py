#!/usr/bin/env python3
"""Rebuild Whiplash's Zero Suit Samus adaptation from pinned scripts and documented authoring rules."""
import copy, hashlib, importlib, json, math, re, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools'))
units = importlib.import_module('units')
SCALE = units.SCALE  # px per source unit: geometry, speeds (units/frame) and accelerations (units/frame²)
# Phase drift (`airAccelMult`) is written as a multiple of the old shared air accels (aerial 0.45, attack 0.25,
# upSpecial 0.3, fitted px/tick² at the old 3.5 horizontal scale, hence × 5 / 3.5). Under `movement.ultimate` it
# multiplies airAccel × the kind's multiplier instead, so drift() stores the value that keeps the same effective
# acceleration.
AIR_ACCEL = round(.095 * SCALE, 5)
def drift(legacy, kind):
    old = {'aerial': 0.45, 'attack': 0.25, 'upSpecial': 0.3}[kind] * 5 / 3.5
    mult = {'aerial': 1.0, 'attack': 0.55, 'upSpecial': 0.65}[kind]
    return round(legacy * old / (AIR_ACCEL * mult), 4)
SOURCES = ROOT / 'docs/imports/sources/whiplash'
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
    elif 180<angle<360: angle-=360;kind='spike' if angle<=-60 else 'normal'
    b={'start':start,'x':num(a,'Z')*5 if x is None else x,'y':num(a,'Y')*5 if y is None else y,'r':num(a,'Size')*5 if r is None else r,'damage':num(a,'Damage'),'bkb':num(a,'BKB'),'kbg':num(a,'KBG')/100,'angle':angle}
    if end is not None:b['end']=end
    if 'collision_attr_paralyze' in a.get('Effect',''):kind='paralyze'
    elif 'collision_attr_bury' in a.get('Effect',''):kind='bury'
    elif 'collision_attr_elec' in a.get('Effect','') and kind=='normal':kind='electric'
    if kind!='normal':b['type']=kind
    if a.get('FacingRestrict','').endswith('_POS'):b['dir']='away'
    if a.get('FacingRestrict','').endswith('_B'):b['dir']='back'
    mask=a.get('Ground_or_Air','')
    if mask.endswith('_G') or mask.endswith('_G_d'):b['situation']='ground'
    elif mask.endswith('_A'):b['situation']='air'
    if num(a,'FKB'):
        # Ultimate WBKB (`fkb`); bkb/kbg stay the source values.
        b['setKb']=True;b['fkb']=num(a,'FKB')
    if num(a,'Rehit'):b['rehit']=int(num(a,'Rehit'))
    if num(a,'Hitlag',1)!=1:b['hitlagMult']=num(a,'Hitlag',1)
    if num(a,'ShieldDamage') and b['damage']>0:b['shieldDmgMult']=max(0,(b['damage']+num(a,'ShieldDamage'))/b['damage'])
    return b

# Fitted animated bone transforms in the 2D gameplay plane (source geometry unit = 5 px).
def bone(slot,a,t,second=False):
    name=a.get('Bone','');x=num(a,'X2' if second else 'X');y=num(a,'Y2' if second else 'Y');z=num(a,'Z2' if second else 'Z')
    if 'top' in name:return z*5,y*5
    if slot=='ftilt':origin,axis=(26,56),(1,0)
    elif slot=='dtilt':origin,axis=(29,10),(1,0)
    elif slot=='fsmash':
        if t<20:origin,axis=((25,51) if 'knee' in name else (9,41)),(1,0)
        else:origin,axis=((70,43) if 'foot' in name else (35,43) if 'knee' in name else (12,36)),(1,0)
    elif slot=='fair':origin,axis=((60,39) if 'foot' in name else (30,36)),(1,0)
    elif slot=='bair':origin,axis=(-26,40),(-1,0)
    elif slot=='uair':
        angle=math.radians(20+(t-6)*140/6);radius=59 if 'foot' in name else 32 if 'knee' in name else 15
        origin,axis=(radius*math.cos(angle),34+radius*math.sin(angle)),(math.cos(angle),math.sin(angle))
    elif slot=='dair':origin,axis=((18,-15) if 'foot' in name else (8,10)),(.25,-.97)
    elif slot=='kick':origin,axis=(25,19),(.707,-.707)
    elif slot in ('grab','dashgrab','zair'):
        if 'throw' in name:
            start=17 if slot=='dashgrab' else 15 if slot=='grab' else 9
            peak=210 if slot=='zair' else 165
            xx=80+(peak-80)*min(1,max(0,(t-start)/5))
            if t>start+5:xx-=max(0,t-start-5)*3
            return xx,45
        return 42,45
    else:origin,axis=(0,40),(1,0)
    return origin[0]+axis[0]*x*5-axis[1]*y*5,origin[1]+axis[1]*x*5+axis[0]*y*5

def boxes(slot,script,shift=0):
    raw=attacks('szerosuit',script);ledger[slot]={'article':'szerosuit','script':script,'attacks':raw}
    out=[]
    for v in raw:
        a=v['source'];lo,hi=v['start']+shift,v['end']+shift
        x,y=bone(slot,a,lo);b=hit(a,lo,hi,x,y)
        capsule=a.get('X2','LUA_VOID')!='LUA_VOID'
        x2,y2=bone(slot,a,lo,True) if capsule else (x,y)
        steps=max(1,math.ceil(math.hypot(x2-x,y2-y)/max(1,b['r']))) if capsule else 0
        for k in range(steps+1):
            c=copy.deepcopy(b);mix=k/max(1,steps);c['x']=x+(x2-x)*mix;c['y']=y+(y2-y)*mix
            if 'top' not in a.get('Bone',''):
                c['frames']=[]
                for t in range(lo,hi+1):
                    q,r=bone(slot,a,t);q2,r2=bone(slot,a,t,True) if capsule else (q,r)
                    c['frames'].append([t,q+(q2-q)*mix,r+(r2-r)*mix,c['r']])
            out.append(c)
    return out

def cut(bs,lo,hi):
    out=[]
    for b in bs:
        if b['start']>hi or b['end']<lo:continue
        c=copy.deepcopy(b);c['start']=max(lo,b['start'])-lo;c['end']=min(hi,b['end'])-lo
        if c.get('frames'):c['frames']=[[t-lo,x,y,r] for t,x,y,r in b['frames'] if lo<=t<=hi]
        out.append(c)
    return out

def phase(name,frames,bs=None,**kw):return dict(name=name,frames=frames,hitboxes=bs or [],**kw)
def timeout(to):return {'on':'timeout','to':to}
def land(lag):return {'on':'land','to':'landing','lag':lag}
def line(a,b,w,c,alpha=1):return {'shape':'line','from':a,'to':b,'width':w,'color':c,'alpha':alpha}
def circle(at,r,color,alpha=1):return {'shape':'circle','at':at,'r':r,'fill':color,'alpha':alpha}
def poly(p,c):return {'shape':'poly','points':p,'fill':c}
stand={'head':[-3,67],'shoulder':[-2,53],'hip':[0,31],'elbowF':[14,43],'handF':[26,48],'elbowB':[-14,45],'handB':[-12,59],'kneeF':[13,17],'footF':[19,0],'kneeB':[-15,14],'footB':[-25,0]}
punch={'head':[-8,65],'shoulder':[-2,50],'hip':[-4,29],'elbowF':[27,50],'handF':[57,50],'elbowB':[-10,56],'handB':[-8,68],'kneeF':[15,14],'footF':[26,0],'kneeB':[-18,13],'footB':[-31,0]}
elbow={'head':[2,62],'shoulder':[5,48],'hip':[-4,26],'elbowF':[42,48],'handF':[20,59],'elbowB':[-10,44],'handB':[-22,49]}
kick={'head':[-12,67],'shoulder':[-8,51],'hip':[0,35],'kneeF':[29,56],'footF':[65,56],'kneeB':[-2,16],'footB':[-5,0],'elbowF':[-18,47],'handF':[-29,51],'elbowB':[-15,61],'handB':[-28,74]}
sweep={'head':[-25,26],'shoulder':[-14,18],'hip':[3,13],'kneeF':[29,10],'footF':[65,10],'kneeB':[-10,5],'footB':[-22,0],'elbowF':[-27,11],'handF':[-37,2],'elbowB':[-18,33],'handB':[-28,39]}
handstand={'head':[0,27],'shoulder':[0,40],'hip':[0,66],'kneeF':[27,70],'footF':[50,70],'kneeB':[-27,70],'footB':[-50,70],'elbowF':[13,22],'handF':[15,1],'elbowB':[-13,22],'handB':[-15,1]}
vertical={**handstand,'kneeF':[4,84],'footF':[3,108],'kneeB':[-4,84],'footB':[-3,108]}
backkick={'head':[13,65],'shoulder':[11,51],'hip':[3,32],'kneeB':[-26,40],'footB':[-62,40],'kneeF':[14,17],'footF':[19,0],'elbowF':[25,52],'handF':[32,62]}
airkick={'head':[-20,65],'shoulder':[-14,51],'hip':[1,34],'kneeF':[31,36],'footF':[61,39],'kneeB':[-14,23],'footB':[-28,16],'elbowF':[0,61],'handF':[14,73],'elbowB':[-28,43],'handB':[-35,51]}
shoot={'head':[-5,66],'shoulder':[-2,52],'hip':[-5,30],'elbowF':[20,47],'handF':[42,45],'elbowB':[11,44],'handB':[34,43]}
upshot={'head':[-4,68],'shoulder':[-2,53],'hip':[-2,31],'elbowF':[7,75],'handF':[10,94],'elbowB':[-11,66],'handB':[-18,78]}
dive={'head':[-8,64],'shoulder':[-3,50],'hip':[0,33],'kneeF':[8,10],'footF':[18,-15],'kneeB':[-10,24],'footB':[-13,12],'elbowF':[8,52],'handF':[3,65],'elbowB':[-15,48],'handB':[-9,60]}
flipkick={'head':[-13,64],'shoulder':[-6,48],'hip':[0,32],'kneeF':[25,19],'footF':[61,-16],'kneeB':[-18,29],'footB':[-33,38],'elbowF':[12,55],'handF':[8,68],'elbowB':[-21,48],'handB':[-25,61]}
poses={'stand':stand,'jabOne':punch,'jabTwo':{**punch,'handF':[42,47],'elbowF':[19,46],'handB':[17,64]},'jabThree':elbow,'sideKick':kick,'sweep':sweep,'handstand':handstand,'verticalKick':vertical,'reverseKick':backkick,'airKick':airkick,'shoot':shoot,'upShot':upshot,'dive':dive,'flipKick':flipkick}
# Fitted locomotion states: a compact crouch, trailing airborne limbs, a light absorb-and-rise landing,
# and a stride with her weight over the planted foot. Authored for this game; the reference GIFs above
# only establish attack poses, so these are not claimed as source bone animation.
crouch={'from':'stand','head':[-4,52],'shoulder':[-3,40],'hip':[0,20],'elbowF':[10,32],'handF':[22,36],'elbowB':[-11,34],'handB':[-10,45],'kneeF':[16,10],'footF':[24,0],'kneeB':[-14,8],'footB':[-22,0],'pivot':24}
air={'from':'stand','head':[-3,62],'shoulder':[-2,49],'hip':[0,30],'elbowF':[17,42],'handF':[30,48],'elbowB':[-17,44],'handB':[-28,56],'kneeF':[22,26],'footF':[34,18],'kneeB':[-17,21],'footB':[-30,12]}
airFall={'from':'locomotionAir','head':[-2,60],'shoulder':[-1,48],'kneeF':[18,18],'footF':[28,6],'kneeB':[-20,16],'footB':[-34,8],'elbowF':[20,44],'handF':[34,52],'elbowB':[-20,46],'handB':[-32,58],'pivot':27}
airSway={'from':'airFall','head':[-4,59],'handF':[32,54],'handB':[-34,56],'footF':[26,8],'footB':[-32,10]}
riseA={'from':'stand','head':[-1,71],'shoulder':[0,56],'hip':[0,33],'elbowF':[13,66],'handF':[17,84],'elbowB':[-14,64],'handB':[-19,82],'kneeF':[15,20],'footF':[21,4],'kneeB':[-15,18],'footB':[-25,2],'pivot':33}
fallA={'from':'airFall','head':[-2,61],'kneeF':[26,24],'footF':[36,14],'kneeB':[-22,16],'footB':[-34,4]}
landA={'from':'locomotionCrouch','head':[-9,46],'shoulder':[-5,34],'hip':[0,14],'elbowF':[14,26],'handF':[28,22],'elbowB':[-14,26],'handB':[-26,20],'kneeF':[18,7],'footF':[27,0],'kneeB':[-16,6],'footB':[-26,0],'pivot':18}
landB={'from':'locomotionCrouch','head':[-5,56],'shoulder':[-3,43],'hip':[0,23],'elbowF':[12,36],'handF':[25,40],'elbowB':[-12,38],'handB':[-12,49]}
jumpHalf={'from':'stand','head':[-4,58],'shoulder':[-3,46],'hip':[0,24],'elbowF':[11,36],'handF':[23,40],'elbowB':[-12,38],'handB':[-12,49],'kneeF':[15,12],'footF':[23,0],'kneeB':[-13,10],'footB':[-22,0],'pivot':26}
jumpCoil={'from':'locomotionCrouch','head':[-7,47],'shoulder':[-4,36],'hip':[0,16],'elbowF':[2,28],'handF':[8,18],'elbowB':[-14,30],'handB':[-22,22],'kneeF':[19,8],'footF':[28,0],'kneeB':[-17,7],'footB':[-27,0],'pivot':20}
helplessA={'from':'airFall','head':[-4,58],'elbowF':[12,40],'handF':[6,30],'elbowB':[-12,42],'handB':[0,32],'kneeF':[10,16],'footF':[14,2],'kneeB':[-8,14],'footB':[-6,2]}
helplessB={'from':'helplessA','head':[2,58],'handF':[10,32],'handB':[-6,30],'footF':[18,4],'footB':[-12,2]}
shieldA={'from':'locomotionCrouch','head':[-3,54],'shoulder':[-2,42],'elbowF':[14,38],'handF':[8,54],'elbowB':[12,36],'handB':[4,50]}
runA={'from':'stand','head':[-6,64],'shoulder':[-3,50],'hip':[0,30],'elbowF':[-2,44],'handF':[-14,48],'elbowB':[16,42],'handB':[28,40],'kneeF':[24,14],'footF':[36,0],'kneeB':[-18,12],'footB':[-30,4]}
runB={'from':'stand','head':[-4,68],'shoulder':[-2,54],'hip':[0,32],'elbowF':[-4,46],'handF':[-12,50],'elbowB':[10,46],'handB':[20,52],'kneeF':[14,26],'footF':[18,12],'kneeB':[-12,20],'footB':[-18,6]}
runC={'from':'stand','head':[-7,64],'shoulder':[-4,50],'hip':[0,30],'elbowF':[16,44],'handF':[28,48],'elbowB':[-2,42],'handB':[-14,40],'kneeF':[-14,12],'footF':[-26,4],'kneeB':[20,14],'footB':[32,0]}
runD={'from':'stand','head':[-4,68],'shoulder':[-2,54],'hip':[0,32],'elbowF':[10,46],'handF':[20,50],'elbowB':[-4,44],'handB':[-12,48],'kneeF':[-10,20],'footF':[-16,6],'kneeB':[16,26],'footB':[20,12]}
poses.update(locomotionCrouch=crouch,locomotionAir=air,airFall=airFall,airSway=airSway,riseA=riseA,fallA=fallA,landA=landA,landB=landB,jumpHalf=jumpHalf,jumpCoil=jumpCoil,helplessA=helplessA,helplessB=helplessB,shieldA=shieldA,runA=runA,runB=runB,runC=runC,runD=runD)
def keys(p,start,end,total,air=False):return [{'at':0,'pose':'air' if air else 'stand'},{'at':max(0,start-2),'pose':p,'lerp':True},{'at':end,'pose':p},{'at':min(total-1,end+10),'pose':'air' if air else 'stand','lerp':True}]
moves={}
for slot,name,script,total,p,charge in [('jab','Snap Combination','Attack11',23,'jabOne',None),('ftilt','High Side Kick','AttackS3',28,'sideKick',None),('utilt','Handstand Split','AttackHi3',45,'handstand',None),('dtilt','Sliding Sweep','AttackLw3',29,'sweep',None),('dashatk','Flying Knee','AttackDash',51,'airKick',None),('fsmash','Jet Double Kick','AttackS4',64,'sideKick',9),('usmash','Plasma Fountain','AttackHi4',48,'upShot',4),('dsmash','Paralyzing Burst','AttackLw4',41,'shoot',15)]:
    bs=boxes(slot,script);start=min(b['start'] for b in bs);end=max(b['end'] for b in bs)
    ph=phase('main',total,bs,anim=keys(p,start,end,total),events=[{'at':start,'sfx':'smash' if charge is not None else 'slash'}])
    if charge is not None:ph['charge']={'frame':charge}
    moves[slot]={'name':name,'phases':[ph]}
for slot,rate in [('utilt',4),('fsmash',10),('usmash',3)]:
    for b in moves[slot]['phases'][0]['hitboxes']:b['rehit']=rate
# Part changes allow the final hit immediately after the preceding multi-hit.
for b in moves['usmash']['phases'][0]['hitboxes']:
    if b['start']==28:b['rehit']=2
moves['utilt']['phases'][0]['anim']=[{'at':0,'pose':'stand'},{'at':2,'pose':'handstand','lerp':True},{'at':4,'pose':'handstand'},{'at':9,'pose':'verticalKick','lerp':True},{'at':14,'pose':'handstand','lerp':True},{'at':30,'pose':'stand','lerp':True}]
moves['fsmash']['phases'][0]['anim']=keys('sideKick',13,14,64)[:3]+[{'at':20,'pose':'air','lerp':True},{'at':26,'pose':{**airkick,'kneeF':[35,43],'footF':[80,43],'footB':[-5,0]},'lerp':True},{'at':29,'pose':{**airkick,'kneeF':[35,43],'footF':[80,43],'footB':[-5,0]}},{'at':50,'pose':'stand','lerp':True}]
moves['dashatk']['phases'][0].update(slide=20,events=[{'at':0,'setVel':{'x':10.7143}},{'at':7,'sfx':'slash'}])
moves['dsmash']['phases'][0]['anim']=keys({**shoot,'handF':[42,25],'elbowF':[20,36]},20,24,41)
for key,script,total,p in [('second','Attack12',25,'jabTwo'),('third','Attack13',34,'jabThree')]:
    bs=boxes('jab'+key,script);moves['jab']['phases'].append(phase(key,total,bs,anim=keys(p,min(b['start'] for b in bs),max(b['end'] for b in bs),total)))
moves['jab']['phases'][0]['transitions']=[{'on':'attackHeld','from':6,'to':'second'}]
moves['jab']['phases'][1]['transitions']=[{'on':'attackHeld','from':6,'to':'third'}]

def aerial(slot,name,script,total,p,lag,acstart,acend):
    bs=boxes(slot,script)
    if slot=='fair':
        for b in bs:b['rehit']=5
    parts=[]
    for lo,hi in zip([0,acstart,acend],[acstart,acend,total]):
        if hi<=lo:continue
        pn='initial' if lo==0 and acstart>0 else 'active' if lo==acstart else 'late'
        tr=land(lag) if lo==acstart else {'on':'land','to':'end'}
        if slot=='dair' and lo==acstart:tr={'on':'land','to':'land'}
        anim=[{'at':0,'pose':p if lo==acstart else 'air'}]
        if lo==acstart:anim += [{'at':max(1,min(hi-lo-1,max(b['end'] for b in bs)-lo+5)),'pose':'air','lerp':True}]
        parts.append(phase(pn,hi-lo,cut(bs,lo,hi-1),anim=anim,transitions=[tr]))
    for n in range(len(parts)-1):parts[n]['transitions'].append(timeout(parts[n+1]['name']))
    moves[slot]={'name':name,'air':'aerial','phases':parts}
    return bs
for spec in [('nair','Plasma Wheel','AttackAirN',51,'shoot',8,4,42),('fair','Scissor Kicks','AttackAirF',46,'airKick',10,4,38),('bair','Reverse Jet Heel','AttackAirB',40,'reverseKick',10,4,31),('uair','Somersault Heel','AttackAirHi',34,'air',5,0,25),('dair','Diving Heel','AttackAirLw',67,'dive',24,4,50),('zair','Plasma Tether','AirCatch',49,'shoot',8,10,40)]:aerial(*spec)
# Hold each scissor kick through its hit window; recovery interpolation must not pull the feet away early.
poses['airKickSecond']={**airkick,'kneeB':[30,36],'footB':[60,39],'kneeF':[-7,26],'footF':[-17,16]}
for ph in moves['fair']['phases']:
    if ph['name']=='active':
        ph['anim']=[{'at':0,'pose':'air'},{'at':2,'pose':'airKick','lerp':True},{'at':3,'pose':'airKick'},{'at':6,'pose':'air','lerp':True},{'at':9,'pose':'airKickSecond','lerp':True},{'at':10,'pose':'airKickSecond'},{'at':19,'pose':'air','lerp':True}]
# The somersault pose follows the fitted foot hitbox sweep, without rotating the collision body.
up=moves['uair']['phases'][0];up['anim']=[{'at':0,'pose':'air'}]
for t in [6,8,10,12]:
    a={'Bone':'footr','X':'0','Y':'0','Z':'0'};foot=bone('uair',a,t);knee=bone('uair',{'Bone':'kneer','X':'0','Y':'0','Z':'0'},t)
    up['anim'].append({'at':t,'pose':{'head':[0,18],'shoulder':[0,30],'hip':[0,34],'kneeF':list(knee),'footF':list(foot),'kneeB':[-15,32],'footB':[-24,16],'handF':[18,6],'handB':[-18,9]},'lerp':True})
up['anim'].append({'at':22,'pose':'air','lerp':True})
# Down air stalls, then commits to a diagonal dive; its own landing phase deals the shockwave.
dair=moves['dair']['phases'];dair[0]['events']=[{'at':0,'setVel':{'x':0,'y':.4167}}];dair[0]['hover']=True
stall=phase('stall',10,hover=True,airAccelMult=0,anim=[{'at':0,'pose':'dive'}],transitions=[{'on':'land','to':'land'},timeout('active')])
dair[0]['transitions'][-1]=timeout('stall')
dair[1].update(frames=36,hitboxes=cut(boxes('dair','AttackAirLw'),14,49),airAccelMult=0,fallCap=25,events=[{'at':0,'setVel':{'x':5,'y':-22.5},'sfx':'slash'}])
dair[1]['anim']=[{'at':0,'pose':'dive'}]
dair.insert(1,stall)
landing=boxes('dairLanding','LandingAirLw')
for b in landing:b['end']=3
moves['dair']['phases'].append(phase('land',24,landing,anim=[{'at':0,'pose':'sweep'},{'at':18,'pose':'stand','lerp':True}],events=[{'at':1,'sfx':'land'}]))
# Extended ledge reach is available from the tether check through the active tip sweep.
for ph in moves['zair']['phases']:
    if ph['name']=='initial':ph.update(ledgeGrab=2,tether={'x1':25,'x2':210,'y1':20,'y2':130})
    elif ph['name']=='active':ph.update(ledgeGrab=0,tether={'x1':25,'x2':210,'y1':20,'y2':130})
# Grabs use the moving tip and a hand box only on the first three active frames.
for slot,script,start,end,total in [('grab','Catch',15,25,58),('dashgrab','CatchDash',17,26,66)]:
    bs=[]
    for bonename,r,stop in [('throw',15,end),('handr',10,start+2)]:
        a={'Bone':bonename,'X':'0','Y':'0','Z':'0'};x,y=bone(slot,a,start)
        bs.append({'start':start,'end':stop,'x':x,'y':y,'r':r,'damage':0,'bkb':0,'kbg':0,'angle':0,'type':'grab','frames':[[t,*bone(slot,a,t),r] for t in range(start,stop+1)]})
    moves[slot]={'name':'Running Tether' if slot=='dashgrab' else 'Lashing Tether','phases':[phase('main',total,bs,anim=keys('shoot',start,end,total),slide=12 if slot=='dashgrab' else 0)]}
# Paralyzer's 20f ready period precedes 0..22 held ticks and a 28f release animation.
moves['nspec']={'name':'Paralyzer','bReverse':True,'chargeDamage':0,'phases':[phase('charge',21,charge={'frame':20,'max':22,'button':'special'},anim=[{'at':0,'pose':'shoot'}],transitions=[timeout('release')]),phase('release',27,events=[{'at':0,'spawn':'paralyzer','sfx':'shoot'}],anim=keys('shoot',0,12,27))]}
# Ground/air scripts branch at frame1; all subsequent local windows are source gameplay frames minus1.
for kind,script in [('ground','SpecialS'),('air','SpecialAirS')]:
    bs=boxes('sspec'+kind,script)
    for b in bs:b['rehit']=2
    first=cut(bs,1,29)
    # Choosing up at frame30 changes the finisher while preserving its original global clock.
    base=phase(kind,29,first,anim=[{'at':0,'pose':'shoot'}],ledgeGrab=19 if kind=='air' else None,transitions=[timeout(kind+'Choose')])
    if kind=='air':base['tether']={'x1':25,'x2':200,'y1':20,'y2':130}
    choose=phase(kind+'Choose',1,anim=[{'at':0,'pose':'shoot'}],transitions=[{'on':'upHeld','to':kind+'Up'},timeout(kind+'End')])
    final=cut(bs,31,54)
    normal=phase(kind+'End',24,final,anim=keys('shoot',0,4,24),transitions=[{'on':'timeout','to':'end'}])
    upbs=cut(boxes('sspec'+kind+'Up','SpecialS2' if kind=='ground' else 'SpecialAirS2'),31,54)
    if kind=='air':
        for b in upbs:b['rehit']=10
    rising=phase(kind+'Up',24,upbs,anim=[{'at':0,'pose':'shoot'},{'at':11,'pose':'upShot','lerp':True},{'at':23,'pose':'stand','lerp':True}])
    moves.setdefault('sspec',{'name':'Plasma Whip','bReverse':True,'phases':[phase('start',1,anim=[{'at':0,'pose':'shoot'}],transitions=[{'on':'airborne','to':'air'},timeout('ground')])]})['phases'] += [base,choose,normal,rising]
# Boost Kick's post-11.0 timeline is +2 frames relative to the retained animation scripts.
moves['uspec']={'name':'Boost Kick','air':'upSpecial','phases':[phase('start',1,events=[{'at':0,'setVel':{'x':0}}],anim=[{'at':0,'pose':'sideKick'}],transitions=[{'on':'airborne','to':'air'},timeout('ground')])]}
for kind,script in [('ground','SpecialHi'),('air','SpecialAirHi')]:
    bs=boxes('uspec'+kind,script,2)
    for b in bs:
        if b['start']==12:b['rehit']=4
        if b['start']==28:b['rehit']=1
        if b['start']==34:b['rehit']=4
    ph=phase(kind,76,cut(bs,1,76),airAccelMult=drift(.75, 'upSpecial'),ledgeGrab=16,anim=[{'at':0,'pose':'airKick'},{'at':5,'pose':'airKick'},{'at':11,'pose':'verticalKick','lerp':True},{'at':23,'pose':'verticalKick'},{'at':27,'pose':'airKick','lerp':True},{'at':33,'pose':{**airkick,'kneeF':[28,55],'footF':[55,62.5]},'lerp':True},{'at':34,'pose':{**airkick,'kneeF':[28,55],'footF':[55,62.5]}},{'at':44,'pose':'air','lerp':True}],events=[{'at':5,'setVel':{'y':21.3612,'x':2.2857},'leaveGround':True,'sfx':'jet'},{'at':11,'setVel':{'y':14.2408}},{'at':27,'setVel':{'x':6.8571,'y':1.0954}},{'at':33,'sfx':'smash'}],transitions=[land(30),timeout('helpless')])
    moves['uspec']['phases'].append(ph)
# Flip Jump is an independent one-use resource. The contact trigger below the feet chooses the stomp.
kickbs=boxes('kick','SpecialAirLwKick')
for b in kickbs:b['dir']='forward'
stompbs=boxes('stomp','SpecialAirLwFlip')
for b in stompbs:b['dir']='away'
flipstretch={'head':[-3,70],'shoulder':[-2,54],'hip':[0,32],'kneeF':[3,15],'footF':[5,-3],'kneeB':[-5,15],'footB':[-7,-3],'elbowF':[5,70],'handF':[2,88],'elbowB':[-8,69],'handB':[-6,86],'pivot':38}
fliptuck={'head':[-12,62],'shoulder':[-5,48],'hip':[0,32],'kneeF':[23,44],'footF':[12,28],'kneeB':[13,45],'footB':[-1,28],'elbowF':[15,49],'handF':[23,43],'elbowB':[5,48],'handB':[14,43],'pivot':38}
fliprecover={**fliptuck,'kneeF':[15,22],'footF':[21,4],'kneeB':[-9,22],'footB':[-15,7],'elbowF':[12,50],'handF':[21,43],'elbowB':[-13,50],'handB':[-21,43]}
poses.update(flipStretch=flipstretch,flipTuck=fliptuck,flipRecover=fliprecover)
flipanim=[{'at':0,'pose':'flipRecover'},{'at':5,'pose':{'from':'flipStretch','rot':-55},'lerp':True},{'at':12,'pose':{'from':'flipStretch','rot':-145},'lerp':True},{'at':20,'pose':{'from':'flipStretch','rot':-225},'lerp':True},{'at':28,'pose':{'from':'flipTuck','rot':-310},'lerp':True},{'at':37,'pose':{'from':'flipRecover','rot':-360},'lerp':True}]
# Kick bounce recoil: the fitted flip poses give the rebound its own silhouette instead of a rotated air pose.
flipbounce={**fliptuck,'head':[-18,56],'shoulder':[-10,44],'hip':[0,30],'kneeF':[8,22],'footF':[14,10],'kneeB':[-20,26],'footB':[-34,32],'elbowF':[6,54],'handF':[-4,64],'elbowB':[-24,48],'handB':[-34,58]}
poses['flipBounce']=flipbounce
# State animations: light landing absorb, trailing airborne limbs, a compact coil and a taut fall loop.
anims={
 'idle':{'clock':'world','loop':72,'keys':[{'at':0,'pose':'stand'},{'at':18,'pose':{'from':'stand','head':[-4,68.2],'shoulder':[-3,54.2],'handF':[27,49],'handB':[-13,60]},'lerp':True},{'at':36,'pose':'stand','lerp':True},{'at':54,'pose':{'from':'stand','head':[-2,66],'shoulder':[-1,52]},'lerp':True},{'at':72,'pose':'stand','lerp':True}]},
 'crouch':{'clock':'world','loop':54,'keys':[{'at':0,'pose':'locomotionCrouch'},{'at':27,'pose':{'from':'locomotionCrouch','head':[-2,53.4],'shoulder':[-1,41.4]},'lerp':True},{'at':54,'pose':'locomotionCrouch','lerp':True}]},
 'run':{'clock':'run','loop':360,'keys':[{'at':0,'pose':'runA'},{'at':45,'pose':'runB','lerp':True},{'at':90,'pose':'runC','lerp':True},{'at':135,'pose':'runD','lerp':True},{'at':180,'pose':'runA','lerp':True},{'at':225,'pose':'runB','lerp':True},{'at':270,'pose':'runC','lerp':True},{'at':315,'pose':'runD','lerp':True},{'at':360,'pose':'runA','lerp':True}]},
 'jumpsquat':{'keys':[{'at':0,'pose':'jumpHalf'},{'at':2,'pose':'jumpCoil','lerp':True}]},
 'landing':{'keys':[{'at':0,'pose':'landA'},{'at':1,'pose':'landB','lerp':True},{'at':2,'pose':{'from':'stand','head':[-3,65],'shoulder':[-2,52],'hip':[0,30]},'lerp':True},{'at':9,'pose':'stand','lerp':True}]},
 'air.rise':{'keys':[{'at':0,'pose':'riseA'},{'at':5,'pose':'locomotionAir','lerp':True}]},
 'air.fall':{'clock':'world','loop':54,'keys':[{'at':0,'pose':'fallA'},{'at':27,'pose':'airSway','lerp':True},{'at':54,'pose':'fallA','lerp':True}]},
 'air.flip':{'loop':20,'keys':[{'at':0,'pose':{'from':'flipTuck','rot':0}},{'at':20,'pose':{'from':'flipTuck','rot':360},'lerp':True}]},
 'helpless':{'clock':'world','loop':44,'keys':[{'at':0,'pose':'helplessA'},{'at':22,'pose':'helplessB','lerp':True},{'at':44,'pose':'helplessA','lerp':True}]},
 'shield':{'keys':[{'at':0,'pose':'shieldA'}]},
}

moves['dspec']={'name':'Flip Jump','oncePerAirtime':True,'phases':[
 phase('flip',38,airAccelMult=drift(3, 'attack'),passPlatforms=True,ledgeGrab=13,detection={'x1':-16,'x2':16,'y1':-22,'y2':5},events=[{'at':0,'setVel':{'x':10.7143,'y':10.5},'leaveGround':True,'sfx':'jump'},{'at':3,'invuln':10},{'at':38,'turn':True}],anim=flipanim,transitions=[{'on':'attackPress','from':14,'to':'kick'},{'on':'specialPress','from':14,'to':'kick'},{'on':'targetNear','from':19,'to':'stomp'},timeout('end')]),
 phase('kick',59,kickbs,airAccelMult=drift(.3, 'attack'),fallCap=16.6667,anim=[{'at':0,'pose':'flipTuck'},{'at':7,'pose':'flipTuck'},{'at':9,'pose':'flipKick','lerp':True},{'at':12,'pose':'flipKick'},{'at':18,'pose':'flipTuck','lerp':True},{'at':28,'pose':'flipTuck'},{'at':38,'pose':'flipRecover','lerp':True},{'at':50,'pose':'air','lerp':True}],events=[{'at':0,'setVel':{'x':10,'y':-2.5}},{'at':9,'setVel':{'x':14.2857,'y':-6.6667},'sfx':'smash'}],transitions=[{'on':'hit','to':'kickBounce'}, {'on':'land','to':'land'}]),
 phase('stomp',58,stompbs,airAccelMult=drift(.8, 'attack'),ledgeGrab=2,anim=[{'at':0,'pose':'dive'},{'at':10,'pose':{'from':'air','rot':-180},'lerp':True},{'at':35,'pose':'air','lerp':True}],events=[{'at':0,'setVel':{'x':2.8571,'y':10},'sfx':'djump'}],transitions=[{'on':'land','to':'end'}]),
 phase('kickBounce',49,airAccelMult=drift(.8, 'attack'),ledgeGrab=1,anim=[{'at':0,'pose':'flipBounce'},{'at':20,'pose':'air','lerp':True}],events=[{'at':0,'setVel':{'x':-4.2857,'y':9.1667}}],transitions=[{'on':'land','to':'land'}]),
 phase('land',35,anim=[{'at':0,'pose':'sweep'},{'at':23,'pose':'stand','lerp':True}]) ]}
throws={}
for key,script,release,total,pre,dmg in [('f','ThrowF',9,25,8,5),('b','ThrowB',11,27,9,2),('u','ThrowHi',5,40,3,2),('d','ThrowLw',22,51,20,4)]:
    a=args(re.search(r'ATTACK_ABS\((Kind=FIGHTER_ATTACK_ABSOLUTE_KIND_THROW,.*?)\)\r?\n',scripts['szerosuit'][script])[1]);a.update(Size='1',X='0',Y='0',Z='0')
    throws[key]={'release':release,'total':total,'hit':hit(a),'ticks':[{'at':pre,'damage':dmg}]}
# Crouched firing stance, compact blaster, jet heels and a long bright hair ribbon identify Whiplash.
def lash_shapes(start,end):
    points=[]
    dx,dy=end[0]-start[0],end[1]-start[1]
    length=math.hypot(dx,dy)
    for i in range(25):
        t=i/24
        wave=math.sin(t*math.pi*3)*math.sin(t*math.pi)*12
        points.append([start[0]+dx*t-wave*dy/length, start[1]+dy*t+wave*dx/length])
    shapes=[]
    for i in range(24):
        taper=1-i/32
        shapes.extend([line(points[i],points[i+1],7*taper,'#ffb52d',.35),line(points[i],points[i+1],3*taper,'#fff0a8')])
    shapes.append(circle(end,4,'#fff3aa'))
    return shapes

attachments=[
 {'id':'ribbon','joint':'head','behind':True,'shapes':[line([-6,5],[-20,10],7,'#f2df82'),line([-20,10],[-35,3],6,'#e6c862'),poly([[-19,13],[-40,7],[-31,-1]],'#f2df82')]},
 {'id':'blaster','joint':'handF','angle':0,'shapes':[poly([[0,-4],[18,-4],[22,2],[18,6],[0,6]],'#314463'),line([3,3],[19,3],2,'#e2e5ed'),circle([20,1],3,'#ffe77d')]},
 {'id':'heelF','joint':'footF','shapes':[line([-5,0],[7,0],5,'#e5cd72'),circle([2,-2],3,'#a5e9ff')]},
 {'id':'heelB','joint':'footB','shapes':[line([-5,0],[7,0],5,'#e5cd72'),circle([2,-2],3,'#a5e9ff')]},
 {'id':'whip','joint':'root','hidden':True,'shapes':lash_shapes([0,0],[160,0])},
 {'id':'whipUp','joint':'root','hidden':True,'shapes':lash_shapes([10,80],[10,170])},
 {'id':'lashFront','joint':'root','hidden':True,'shapes':[line([30,55],[95,35],4,'#ffc347'),line([30,55],[95,35],1.5,'#fff2c0')]},
 {'id':'lashBack','joint':'root','hidden':True,'shapes':[line([-15,65],[-75,82.5],4,'#ffc347'),line([-15,65],[-75,82.5],1.5,'#fff2c0')]}
]
for slot in ['grab','dashgrab','zair']:
 for ph in moves[slot]['phases']:
    if ph['hitboxes']:
        lo,hi=min(b['start'] for b in ph['hitboxes']),max(b['end'] for b in ph['hitboxes'])
        tips=[b for b in ph['hitboxes'] if b['r']==15]
        keys=next((b['frames'] for b in tips if b.get('frames')),[])
        ph['attach']={'whip':{'show':[lo,hi],'offset':[[lo,35,45]],'scale':[[t,max(.1,(x-35)/160)] for t,x,y,r in keys]}}
for ph in moves['sspec']['phases']:
 if ph['name'].endswith('Up'):ph['attach']={'whip':{'show':[0,9],'offset':[[0,35,45]],'scale':[[0,1],[9,.5]]},'whipUp':{'show':[10,26]}}
 elif ph['hitboxes']:
    lo,hi=min(b['start'] for b in ph['hitboxes']),max(b['end'] for b in ph['hitboxes'])
    ph['attach']={'whip':{'show':[lo,hi],'offset':[[lo,35,45]],'scale':([[lo,.97],[hi,.97]] if ph['name'].endswith('End') else [[lo,.3],[max(lo+1,hi),1]])}}
moves['usmash']['phases'][0]['attach']={'whipUp':{'show':[10,29]}}
for ph in moves['nair']['phases']:
 if ph['name']=='active':ph['attach']={'lashFront':{'show':[6,7]},'lashBack':{'show':[11,12]}}
projectiles={'paralyzer':{'name':'Paralyzer','spawn':[50,45],'vel':[17.1429,0],'r':6.5,'life':24,'max':3,'hit':{'x':0,'y':0,'r':6.5,'damage':4,'bkb':34.1,'kbg':1,'angle':45,'groundAngle':0,'type':'paralyze','setKb':True,'hitlagMult':.7},'chargeScale':{'speed':.014,'damage':.5/22,'bkb':(68.4/34.1-1)/22,'radius':(10/6.5-1)/22,'life':1/22,'angleTo':60,'groundAngleTo':-45},'shapes':[circle([0,0],6.5,'#fff3b5'),line([-12,0],[5,0],3,'#ffce4b')],'trail':[{'every':2,'fx':[{'type':'part','x':0,'y':0,'vx':-.8,'vy':0,'life':8,'size':3,'color':'#ffed9a'}]}]}}
for slot in ['grab','dashgrab']:
    ph=moves[slot]['phases'][0]
    track=ph['attach']['whip']
    end=track['show'][1]
    track['show'][1]=end+12
    track['scale'].extend([[end+6, .35], [end+12, .02]])

# Source jump heights (units): full 44.5, short 21.35, air 44.2 (ssbwiki Jump, SSBU jump height ranking).
# runAccel .6429 is fitted (old .45 × 5 / 3.5).
GRAVITY=round(.12*SCALE,5)
fighter={'$schema':'../../schema/fighter.schema.json','schemaVersion':1,'extends':'base','name':'Whiplash','tagline':'Never where you expected','blurb':'A Zero Suit Samus-inspired acrobat: swift kicks, plasma tethers, paralysis and a slippery Flip Jump. Use grab in the air for a tether attack; press attack or special during Flip Jump for its diving kick.','stats':{'weight':1.25,'pummelDamage':1.3,'walkSpeed':round(1.47*SCALE,5),'runSpeed':round(2.31*SCALE,5),'dashSpeed':round(2.42*SCALE,5),'dashFrames':13,'runAccel':.6429,'groundFriction':.84,'airSpeed':round(1.26*SCALE,5),'airAccel':AIR_ACCEL,'gravity':GRAVITY,'fallSpeed':round(1.7*SCALE,5),'fastFallSpeed':round(2.72*SCALE,5),'traction':round(0.116*SCALE,5),'airFriction':round(0.008*SCALE,5),**units.jumps(44.5,21.35,44.2,GRAVITY),'airJumpDrift':round(1.26*SCALE,5)},'body':{'halfWidth':13,'height':74},'hurtboxes':{'default':{'x1':-14,'x2':14,'y1':0,'y2':77},'knockdown':{'x1':-29,'x2':29,'y1':0,'y2':16}},'style':{'torsoWidth':5,'frontLimbWidth':5,'backLimbWidth':4.5},'ai':{'reach':72,'counterChance':0,'specials':[{'minDistance':160,'maxDistance':390,'height':75,'chance':.2,'grounded':True,'noProjectile':False,'down':False,'hold':[0,22]}]},'poses':poses,'anims':anims,'moves':moves,'throws':throws,'projectiles':projectiles,'attachments':attachments}
(ROOT/'assets/fighters/whiplash/fighter.json').write_text(json.dumps(fighter,indent=2)+'\n')
(SOURCES/'ledger.json').write_text(json.dumps({'sourceCommit':source['commit'],'scale':SCALE,'rawAttackLedger':ledger,'gameplay':{'url':'https://ultimateframedata.com/zero_suit_samus','accessed':'2026-11-02','moves':{slot:{'name':m['name'],'phases':[(p['name'],p['frames']) for p in m['phases']]} for slot,m in moves.items()}}},indent=2)+'\n')
print('Rebuilt Whiplash from pinned scripts and fitted poses; run just bless to normalize content.')
