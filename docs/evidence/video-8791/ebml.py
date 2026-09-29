import sys, collections
MASTER={0x1A45DFA3,0x18538067,0x114D9B74,0x4DBB,0x1549A966,0x1654AE6B,0xAE,0xE0,0xE1,0x1F43B675,0xA0,0x1C53BB6B,0xBB,0xB7,0x1254C367,0x7373,0x63C0,0x67C8,0x6D80,0x6240,0x5035,0xA6,0x75A1,0x8E}
NAMES={0xA3:'SimpleBlock',0xA0:'BlockGroup',0xA1:'Block',0x1C53BB6B:'Cues',0x114D9B74:'SeekHead',0x1F43B675:'Cluster',0x75A1:'BlockAdditions',0xFB:'ReferenceBlock',0x9B:'BlockDuration',0x75A2:'DiscardPadding',0x56AA:'CodecDelay',0x56BB:'SeekPreRoll',0x6D80:'ContentEncodings',0x9C:'FlagLacing',0x4282:'DocType',0x2AD7B1:'TimecodeScale',0x63A2:'CodecPrivate',0x53C0:'AlphaMode',0x55B0:'Colour',0xA7:'Position',0xAB:'PrevSize',0x1254C367:'Tags',0x1043A770:'Chapters',0x1941A469:'Attachments',0xEC:'Void'}
def vint(b,p,keep=False):
    f=b[p]; l=1; m=0x80
    while l<=8 and not f&m: l+=1; m>>=1
    v=f if keep else f&(m-1)
    for i in range(1,l): v=(v<<8)|b[p+i]
    return v,l
def walk(b,s,e,st,doc):
    p=s
    while p<e:
        i,il=vint(b,p,True); p+=il
        sz,sl=vint(b,p); p+=sl
        unk = sz==(1<<(7*sl))-1
        if unk: sz=e-p
        n=NAMES.get(i)
        if n: st[n]+=1
        if i==0x4282: doc.append(b[p:p+sz].decode())
        if i in (0xA3,0xA1):
            tr,tl=vint(b,p); flags=b[p+tl+2]; lace=(flags>>1)&3
            st['track%d_%s'%(tr,n)]+=1
            if lace: st['laced_%d'%lace]+=1
            if i==0xA3 and flags&0x80: st['track%d_key'%tr]+=1
            if i==0xA3 and flags&0x08: st['invisible']+=1
            if tr==1:
                d=b[p+tl+3:p+sz]
                if d:
                    last=d[-1]
                    if last&0xe0==0xc0: st['vp9_superframe']+=1
                    # show_existing / invisible frames via header bit
            else:
                if sz-(tl+3)==0: st['track%d_emptyblock'%tr]+=1
        if i in MASTER: walk(b,p,p+sz,st,doc)
        p+=sz
for f in sys.argv[1:]:
    b=open(f,'rb').read(); st=collections.Counter(); doc=[]
    walk(b,0,len(b),st,doc)
    print(f.split('/')[-1],doc,dict(sorted(st.items())))
