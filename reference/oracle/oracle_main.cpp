// MMANA reference oracle.
//
// Runs the ORIGINAL MININEC3 engine (generated/mininec3.cpp, unmodified) on a
// .MAA file and prints every intermediate the Rust port must reproduce as JSON.
// The load/stack/calc sequence below is adapted from TMainWnd (MainAntIO.cpp
// LoadAntStrings, Main.cpp InitAntDef/GetAntDef/Stack/AddWireNo/
// RunCurrentCalculation) with the VCL form fields replaced by plain variables.
//
// usage: oracle <file.maa> [--freq MHz] [--ground 0|1|2] [--height m] [--near E|H] [--far 1]
//        [--media rel,cond,intval,height;...] [--screen count,radius]
// (the real ground's media and radial screen are program settings, not part
// of the file; `-` leaves an interval or height unset)
#include "ComLib.h"
#include "mininec3.h"
#include "Main.h"

ENVDEF env;
EXEENV exeenv;
CRes res;
TMainWndShim MainWndObj;
TMainWndShim *MainWnd = &MainWndObj;

static ANTDEF ant;
static ANTDEF calAnt;
static ANTDEF *pCalAnt = &ant;

extern double *aCR;
extern double *aCI;

void WarningMB(LPCSTR fmt, ...)
{
	va_list pp;
	va_start(pp, fmt);
	vfprintf(stderr, fmt, pp);
	va_end(pp);
	fputc('\n', stderr);
}
LPCSTR StrDbl(double d)
{
	static char bf[64];
	snprintf(bf, sizeof(bf), "%g", d);
	return bf;
}
LPCSTR StrCLX(double, double) { return ""; }

// ZSCopy without the Shift_JIS full-width conversion (sample files are ASCII).
void ZSCopy(LPSTR t, LPCSTR p, int n)
{
	for( ; *p && n; p++, t++, n-- ) *t = *p;
	*t = 0;
}

// --- Main.cpp TMainWnd::FormCreate defaults ---------------------------------
static void InitEnv(void)
{
	memset(&env, 0, sizeof(env));
	env.antheight = 20.0;
	env.mmax = 1;
	for( int i = 0; i < EMAX; i++ ){
		env.rel[i] = 5.0;
		env.cond[i] = 1.0E-3;
		env.intval[i] = NULLV;
		env.height[i] = NULLV;
	}
	env.gscrcnt = 100;
	env.gscrr = 5;
	env.fbr = 120 + (60<<16);
	env.RO = 50.0;
	env.JXO = 0.0;
	env.WireRoss = 0;
	env.pmax = 4096;

	memset(&exeenv, 0, sizeof(exeenv));
	exeenv.rmd = 1.0;
	exeenv.FixFreeAngle = 1;
}

// --- Main.cpp TMainWnd::InitAntDef ------------------------------------------
static void InitAntDef(void)
{
	memset(&ant, 0, sizeof(ant));
	ant.cauto = 1;
	ant.lenb = 1;
	ant.fq = 14.050;
	ant.DM1 = 400;
	ant.DM2 = 40;
	ant.EC = 1;
	ant.SC = 2.0;
	ant.StackVT = 0;
	ant.StackH = ant.StackV = 1;
	exeenv.rmd = 299.8/ant.fq;
	ant.StackHW = ant.StackVW = exeenv.rmd;
	env.fbr = 120 + (60<<16);
	env.RO = 50.0;
	env.JXO = 0.0;
	for( int i = 0; i < LMAX; i++ ){
		for( int j = 0; j < SMAX; j++ ){
			ant.ldef[i].A[j] = NULLV;
			ant.ldef[i].B[j] = NULLV;
		}
	}
}

// --- MainAntIO.cpp TMainWnd::LoadAntStrings (UI and comment handling removed)
static void LoadAntStrings(const char *in)
{
	CTextString	cs(in);
	int		i, j;
	char	bf[1024];
	LPSTR	t, p;

	if( cs.LoadTextLoop(bf, 1023) == FALSE ) goto _ex;
	strncpy(ant.Name, bf, sizeof(ant.Name));
	if( cs.LoadTextLoop(bf, 1023) == FALSE ) goto _ex;
	Calc(ant.fq, bf);
	if( cs.LoadTextLoop(bf, 1023) == FALSE ) goto _ex;
	p = StrDlm(t, bf);
	CalcU(ant.wmax, t);
	p = StrDlm(t, p);
	if( *t ){		// スタック設定
		CalcU(ant.StackH, t);
		p = StrDlm(t, p);
		CalcU(ant.StackV, t);
		p = StrDlm(t, p);
		Calc(ant.StackHW, t);
		p = StrDlm(t, p);
		Calc(ant.StackVW, t);
		StrDlm(t, p);
		CalcU(ant.StackVT, t);
		if( (ant.StackH < 1)||(ant.StackV < 1) ){
			ant.StackH = ant.StackV = 1;
		}
	}
	else {
		ant.StackVT = 0;
		ant.StackH = ant.StackV = 1;
	}
	if( (ant.StackH == 1) && (ant.StackV == 1) && ant.fq ){
		ant.StackHW = ant.StackVW = 299.8/ant.fq;
	}
	if( ant.wmax > WMAX ) ant.wmax = WMAX;
	for( i = 0; i < ant.wmax; i++ ){
		if( cs.LoadTextLoop(bf, 1023) == FALSE ) goto _ex;
		p = StrDlm(t, bf);	Calc(ant.wdef[i].X1, t);
		p = StrDlm(t, p);	Calc(ant.wdef[i].Y1, t);
		p = StrDlm(t, p);	Calc(ant.wdef[i].Z1, t);
		p = StrDlm(t, p);	Calc(ant.wdef[i].X2, t);
		p = StrDlm(t, p);	Calc(ant.wdef[i].Y2, t);
		p = StrDlm(t, p);	Calc(ant.wdef[i].Z2, t);
		p = StrDlm(t, p);	Calc(ant.wdef[i].R, t);
		p = StrDlm(t, p);	CalcI(ant.wdef[i].SEG, t);
		StrDlm(t, p);
		if( *t ){
			CalcI(j, t);
			if( j ) ant.wdef[i].SEG = 0x80000000 | ABS(ant.wdef[i].SEG) | (ABS(j) << 16);
		}
	}
	if( cs.LoadTextLoop(bf, 1023) == FALSE ) goto _ex;
	p = StrDlm(t, bf);
	CalcU(ant.cmax, t);
	StrDlm(t, p);
	CalcU(ant.cauto, t); if( ant.cauto ) ant.cauto = 1;
	if( ant.cmax > CMAX ) ant.cmax = CMAX;
	for( i = 0; i < ant.cmax; i++ ){
		if( cs.LoadTextLoop(bf, 1023) == FALSE ) goto _ex;
		p = StrDlm(t, bf);	ZSCopy(ant.cdef[i].PLUS, t, 8);
		p = StrDlm(t, p);	Calc(ant.cdef[i].DEG, t);
		StrDlm(t, p);		Calc(ant.cdef[i].EV, t);
	}
	if( cs.LoadTextLoop(bf, 1023) == FALSE ) goto _ex;
	p = StrDlm(t, bf);
	CalcU(ant.lmax, t);
	StrDlm(t, p);
	CalcU(ant.lenb, t); if( ant.lenb ) ant.lenb = 1;
	if( ant.lmax > LMAX ) ant.lmax = LMAX;
	for( i = 0; i < ant.lmax; i++ ){
		if( cs.LoadTextLoop(bf, 1023) == FALSE ) goto _ex;
		p = StrDlm(t, bf);	ZSCopy(ant.ldef[i].PLUS, t, 8);
		p = StrDlm(t, p);	CalcU(ant.ldef[i].TYPE, t);
		switch(ant.ldef[i].TYPE){
			case 0:
				p = StrDlm(t, p);	Calc(ant.ldef[i].A[0], t);
				p = StrDlm(t, p);	Calc(ant.ldef[i].B[0], t);
				StrDlm(t, p);		Calc(ant.ldef[i].A[1], t);
				break;
			case 1:
				p = StrDlm(t, p);	Calc(ant.ldef[i].A[0], t);
				StrDlm(t, p);		Calc(ant.ldef[i].B[0], t);
				break;
			case 2:
				for( j = 0; j < SMAX; j++ ){
					p = StrDlm(t, p);
					if( !*t ) break;
					Calc(ant.ldef[i].A[j], t);
					p = StrDlm(t, p);
					Calc(ant.ldef[i].B[j], t);
				}
				ant.ldef[i].SN = j;
				for( ; j < SMAX; j++ ){
					ant.ldef[i].A[j] = NULLV;
					ant.ldef[i].B[j] = NULLV;
				}
				break;
		}
	}
	if( cs.LoadTextLoop(bf, 1023) == TRUE ){
		p = StrDlm(t, bf);	CalcU(ant.DM1, t);
		p = StrDlm(t, p);	CalcU(ant.DM2, t);
		p = StrDlm(t, p);	Calc(ant.SC, t);
		if( (ant.SC <= 1.0) || (ant.SC > 3.0) ) ant.SC = 2.0;
		StrDlm(t, p);
		if( *t ){
			CalcU(ant.EC, t);
			if( ant.EC <= 0 ) ant.EC = 1;
		}
		else {
			ant.EC = 1;
		}
	}
	else {
		ant.DM2 = 400;
		ant.DM2 = 40;
		ant.EC = 1;
		ant.SC = 2.0;
	}
	if( cs.LoadTextLoop(bf, 1023) == FALSE ) goto _ex;
	p = StrDlm(t, bf);
	CalcU(env.type, t);
	if( env.type >= 3 ) env.type = 0;
	p = StrDlm(t, p);
	Calc(env.antheight, t);
	p = StrDlm(t, p);
	if( *t ){
		CalcU(env.WireRoss, t);
		if( (env.WireRoss < 0)||(env.WireRoss > 6) ) env.WireRoss = 0;
		p = StrDlm(t, p);
		if( *t && (Calc(env.RO, t)==TRUE) ){
			p = StrDlm(t, p);	CalcU(i, t);
			p = StrDlm(t, p);	CalcU(j, t);
			env.fbr = i | (j << 16);
			StrDlm(t, p);
			if( *t ) Calc(env.JXO, t);
		}
	}
	else {
		env.WireRoss = 0;
	}
	while(1){
		if( cs.LoadTextLoop(bf, 1023) == FALSE ) goto _ex;
		if( !strncmp(bf, "$$$", 3) ){
			if( cs.LoadTextLoop(bf, 1023) == FALSE ) goto _ex;
			CalcU(ant.pmax, bf);
			for( i = 0; (i < ant.pmax)&&(i < PTMAX); i++ ){
				if( cs.LoadTextLoop(bf, 1023) == FALSE ) goto _ex;
				p = StrDlm(t, bf);	Calc(ant.pdef[i].RR, t);
				p = StrDlm(t, p);	CalcU(ant.pdef[i].Type, t);
				for( j = 0; j < PPMAX; j++ ){
					p = StrDlm(t, p);
					if( !*t ) break;
					Calc(ant.pdef[i].L[j], t);
					p = StrDlm(t, p);
					Calc(ant.pdef[i].R[j], t);
				}
				for( ; j < PPMAX; j++ ){
					ant.pdef[i].R[j] = 0.007;
				}
			}
			AdjPdef(&ant);
		}
		else if( !strncmp(bf, "###", 3) ){
			break;		// comment block: not needed by the engine
		}
	}
_ex:;
}

// --- Main.cpp TMainWnd::AddWireNo -------------------------------------------
static void AddWireNo(LPSTR t, int off)
{
	if( !off ) return;
	int		d;
	LPSTR	p;
	if( (*t == 'w')||(*t == 'W') ){
		char	bf[32];
		for( p = t+1; *p; p++ ){
			if( !isdigit(*p) ) break;
		}
		strcpy(bf, p);
		*p = 0;
		sscanf(t+1, "%d", &d);
		sprintf(t+1, "%d%s", d + off, bf);
	}
	else {
		sscanf(t, "%d", &d);
		sprintf(t, "%d", d + off);
	}
}

// --- Main.cpp TMainWnd::Stack -----------------------------------------------
static void Stack(void)
{
	memcpy(pCalAnt, &ant, sizeof(ANTDEF));
	ANTDEF	*bp = new ANTDEF;
	int i, n;
	WDEF	*wsp, *wtp;

	int		wmax = pCalAnt->wmax;
	memcpy(bp, pCalAnt, sizeof(ANTDEF));
	if( ant.StackH > 1 ){
		double	yc = -((ant.StackH-1) * ant.StackHW)/2;
		wtp = pCalAnt->wdef;
		for( i = 0; i < ant.StackH; i++, yc += ant.StackHW ){
			for( wsp = bp->wdef, n = 0; n < bp->wmax; n++, wsp++, wtp++ ){
				memcpy(wtp, wsp, sizeof(WDEF));
				wtp->Y1 += yc;
				wtp->Y2 += yc;
			}
		}
		pCalAnt->wmax *= ant.StackH;
	}
	memcpy(bp, pCalAnt, sizeof(ANTDEF));
	if( ant.StackV > 1 ){
		double	zc;
		switch(ant.StackVT){
			case 1:		zc = 0; break;
			case 2:		zc = -((ant.StackV-1) * ant.StackVW); break;
			default:	zc = -((ant.StackV-1) * ant.StackVW)/2; break;
		}
		wtp = pCalAnt->wdef;
		for( i = 0; i < ant.StackV; i++, zc += ant.StackVW ){
			for( wsp = bp->wdef, n = 0; n < bp->wmax; n++, wsp++, wtp++ ){
				memcpy(wtp, wsp, sizeof(WDEF));
				wtp->Z1 += zc;
				wtp->Z2 += zc;
			}
		}
		pCalAnt->wmax *= ant.StackV;
	}
	CDEF	*csp;
	CDEF	*ctp = pCalAnt->cdef;
	int coff = 0;
	for( i = 0; i < (ant.StackV * ant.StackH); i++, coff += wmax ){
		for( csp = bp->cdef, n = 0; n < bp->cmax; n++, csp++, ctp++ ){
			memcpy(ctp, csp, sizeof(CDEF));
			AddWireNo(ctp->PLUS, coff);
			ctp->EV /= (ant.StackV * ant.StackH);
		}
	}
	pCalAnt->cmax *= (ant.StackV * ant.StackH);
	LDEF	*lsp;
	LDEF	*ltp = pCalAnt->ldef;
	int loff = 0;
	for( i = 0; i < (ant.StackV * ant.StackH); i++, loff += wmax ){
		for( lsp = bp->ldef, n = 0; n < bp->lmax; n++, lsp++, ltp++ ){
			memcpy(ltp, lsp, sizeof(LDEF));
			AddWireNo(ltp->PLUS, loff);
		}
	}
	pCalAnt->lmax *= (ant.StackV * ant.StackH);
	delete bp;
}

// --- JSON output -------------------------------------------------------------
static void PutStr(const char *key, const char *s)
{
	printf("\"%s\":\"", key);
	for( ; *s; s++ ){
		unsigned char c = (unsigned char)*s;
		if( (c == '"') || (c == '\\') ) printf("\\%c", c);
		else if( c < 0x20 ) printf("\\u%04x", c);
		else if( c >= 0x80 ) printf("\\u%04x", c);	// raw CP932 byte, not compared
		else putchar(c);
	}
	printf("\"");
}
static void PutFloatArray(const float *p, int n)
{
	printf("[");
	for( int i = 0; i < n; i++ ) printf("%s%.9g", i ? "," : "", p[i]);
	printf("]");
}

int main(int argc, char **argv)
{
	if( argc < 2 ){
		fprintf(stderr, "usage: oracle <file.maa> [--freq MHz] [--ground 0|1|2] [--height m]\n");
		return 2;
	}
	double optFreq = 0.0, optHeight = -1.0;
	int optGround = -1;
	char optNear = 0;
	int optFar = 0;
	const char *optMedia = NULL, *optScreen = NULL;
	for( int i = 2; i + 1 < argc; i += 2 ){
		if( !strcmp(argv[i], "--freq") ) optFreq = atof(argv[i+1]);
		else if( !strcmp(argv[i], "--ground") ) optGround = atoi(argv[i+1]);
		else if( !strcmp(argv[i], "--height") ) optHeight = atof(argv[i+1]);
		else if( !strcmp(argv[i], "--near") ) optNear = argv[i+1][0];
		else if( !strcmp(argv[i], "--far") ) optFar = atoi(argv[i+1]);
		else if( !strcmp(argv[i], "--media") ) optMedia = argv[i+1];
		else if( !strcmp(argv[i], "--screen") ) optScreen = argv[i+1];
	}

	// TMainWnd::LoadAntFile: fgets per line, ClipLF, append CRLF
	FILE *fp = fopen(argv[1], "rt");
	if( fp == NULL ){
		fprintf(stderr, "cannot open %s\n", argv[1]);
		return 2;
	}
	size_t cap = 1 << 20, len = 0;
	char *in = (char *)malloc(cap);
	char line[1024];
	in[0] = 0;
	while( fgets(line, 1024, fp) != NULL ){
		for( char *s = line; *s; s++ ){
			if( (*s == '\r') || (*s == '\n') ){ *s = 0; break; }
		}
		size_t l = strlen(line);
		if( len + l + 3 >= cap ) break;
		memcpy(in + len, line, l);
		len += l;
		in[len++] = '\r';
		in[len++] = '\n';
		in[len] = 0;
	}
	fclose(fp);

	InitEnv();
	env.pmax = InitNEC(env.pmax);
	InitAntDef();
	LoadAntStrings(in);
	free(in);

	// SetAntDef() puts "%.3lf" of fq into the Freq/DoFreq boxes and
	// GetAntDef() parses them back before every calculation.
	char bf[64];
	double d;
	if( ant.fq ){
		snprintf(bf, sizeof(bf), "%.3lf", ant.fq);
	}
	else {
		bf[0] = 0;
	}
	Calc(d, bf);
	if( d > 0 ) ant.fq = d;
	if( d > 0 ) ant.cfq = d;
	for( int i = 0; i < ant.lmax; i++ ){
		if( ant.ldef[i].TYPE == 2 ){
			int j;
			for( j = SMAX-1; j >= 0; j-- ){
				if( (ant.ldef[i].A[j] != NULLV) || (ant.ldef[i].B[j] != NULLV) ) break;
			}
			ant.ldef[i].SN = j+1;
		}
	}
	if( optFreq > 0 ) ant.cfq = optFreq;
	if( optGround >= 0 ) env.type = optGround;
	if( optHeight >= 0 ) env.antheight = optHeight;
	if( optMedia ){
		env.mmax = 0;
		for( const char *m = optMedia; *m && env.mmax < EMAX; ){
			char f[4][64] = {{0}};
			int k = 0, n = 0;
			for( ; *m && *m != ';'; m++ ){
				if( *m == ',' ){ k++; n = 0; }
				else if( (k < 4) && (n < 63) ) f[k][n++] = *m;
			}
			if( *m == ';' ) m++;
			env.rel[env.mmax] = atof(f[0]);
			env.cond[env.mmax] = atof(f[1]);
			env.intval[env.mmax] = (f[2][0] && strcmp(f[2], "-")) ? atof(f[2]) : NULLV;
			env.height[env.mmax] = (f[3][0] && strcmp(f[3], "-")) ? atof(f[3]) : NULLV;
			env.mmax++;
		}
	}
	if( optScreen ){
		env.mtype = 1;
		sscanf(optScreen, "%d,%lf", &env.gscrcnt, &env.gscrr);
	}
	if( ant.cfq == 0.0 ) ant.cfq = ant.fq;

	// TMainWnd::SetStackAnt
	if( (ant.StackH == 1)&&(ant.StackV == 1) ){
		pCalAnt = &ant;
	}
	else {
		pCalAnt = &calAnt;
		Stack();
	}
	printf("{");
	PutStr("name", ant.Name);
	printf(",\"freq\":%.17g", pCalAnt->cfq);
	if( optMedia || optScreen ){
		// for the test to set up the same ground (null: unset)
		printf(",\"real_ground\":{\"media\":[");
		for( int i = 0; i < env.mmax; i++ ){
			printf("%s[%.17g,%.17g,", i ? "," : "", env.rel[i], env.cond[i]);
			if( env.intval[i] == NULLV ) printf("null,"); else printf("%.17g,", env.intval[i]);
			if( env.height[i] == NULLV ) printf("null]"); else printf("%.17g]", env.height[i]);
		}
		if( env.mtype ) printf("],\"screen\":[%d,%.17g]}", env.gscrcnt, env.gscrr);
		else printf("],\"screen\":null}");
	}
	if( !InitAntData(pCalAnt) ){
		printf(",\"error\":\"InitAntData failed\"}\n");
		return 0;
	}
	int N = GetPlusMax();
	printf(",\"pulses\":%d", N);
	printf(",\"wavelength\":%.17g", pCalAnt->RMD);
	printf(",\"wires\":[");
	for( int i = 0; i < pCalAnt->wmax; i++ ){
		WDEF *wp = &pCalAnt->wdef[i];
		printf("%s{\"pno\":%d,\"pmax\":%d}", i ? "," : "", wp->PNo, wp->PMax);
	}
	printf("],\"wz\":[");
	for( int i = 0; i < pCalAnt->wzmax; i++ ){
		WZDEF *zp = &pCalAnt->wzdef[i];
		printf("%s[%.17g,%.17g,%.17g,%.17g,%.17g,%.17g,%.17g,%d,%d,%d,%d,%d]", i ? "," : "",
			zp->X1, zp->Y1, zp->Z1, zp->X2, zp->Y2, zp->Z2, zp->R, zp->SEG, zp->Wno, zp->PNo, zp->SNo, zp->SMax);
	}
	printf("],\"sources\":[");
	for( int i = 0; i < pCalAnt->cmax; i++ ){
		printf("%s%d", i ? "," : "", pCalAnt->cdef[i].PLUSNo);
	}
	printf("],\"loads\":[");
	for( int i = 0; i < pCalAnt->lmax; i++ ){
		printf("%s%d", i ? "," : "", pCalAnt->ldef[i].PLUSNo);
	}
	printf("]");

	// TMainWnd::RunCurrentCalculation
	res.IncResP();
	res.GetResP()->TYPE = env.type;
	res.GetResP()->HEIGHT = env.antheight;
	res.GetResP()->FBR = env.fbr;
	res.GetResP()->RO = env.RO;
	res.GetResP()->JXO = env.JXO;
	CalCurrent();

	printf(",\"currents\":[");
	for( int i = 1; i <= N; i++ ){
		printf("%s[%.17g,%.17g]", (i > 1) ? "," : "", aCR[i], aCI[i]);
	}
	printf("]");
	RESL *rp = res.GetResP();
	printf(",\"result\":{\"r\":%.9g,\"jx\":%.9g,\"swr\":%.9g,\"gain\":%.9g,\"fb\":%.9g,\"elv\":%.9g,\"maxhp\":%.9g,\"maxvp\":%.9g}",
		rp->R, rp->JX, rp->SWR, rp->GAIN, rp->FB, rp->ELV, rp->MAXHP, rp->MAXVP);
	printf(",\"pattern\":{\"elv_h\":%.9g,\"max_v\":%.9g,\"max_ev\":%.9g,\"max_h\":%.9g,\"h\":[", res.ElvHPtn, res.MaxV, res.MaxEV, res.MaxH);
	for( int k = 0; k < 3; k++ ){
		if( k ) printf(",");
		PutFloatArray(res.PtnH[k], 360);
	}
	printf("],\"v\":[");
	for( int k = 0; k < 3; k++ ){
		if( k ) printf(",");
		PutFloatArray(res.PtnV[k], 181);
	}
	printf("]}");
	printf(",\"cur\":");
	PutFloatArray(res.Cur, res.CurN + 1);
	if( optNear ){
		// TMainWnd near-field export: fixed grid used by the golden tests
		NEARFILE nf;
		memset(&nf, 0, sizeof(nf));
		nf.fp = tmpfile();
		nf.TYPE = (optNear == 'H') ? 1 : 0;
		nf.POW = 100.0;
		nf.X = -2.0; nf.XW = 1.5; nf.XN = 3;
		nf.Y = -1.0; nf.YW = 2.0; nf.YN = 2;
		nf.Z = pCalAnt->MinZ + (env.type ? env.antheight : 0.0) + 1.0; nf.ZW = 3.0; nf.ZN = 2;
		exeenv.pNearFile = &nf;
		SaveNear();
		exeenv.pNearFile = NULL;
		printf(",\"near_spec\":{\"kind\":\"%c\",\"power\":100,\"x\":[-2,1.5,3],\"y\":[-1,2,2],\"z\":[%.17g,3,2]}", optNear, nf.Z);
		printf(",\"near\":[");
		rewind(nf.fp);
		int first = 1;
		while( fgets(line, sizeof(line), nf.fp) != NULL ){
			double x, y, z, re, im, mag, deg;
			char c;
			if( sscanf(line, "%lf,%lf,%lf,%c,%le,%le,%le,%lf", &x, &y, &z, &c, &re, &im, &mag, &deg) == 8 ){
				printf("%s[%.17g,%.17g,%.17g,\"%c\",%.17g,%.17g,%.17g,%.17g]", first ? "" : ",", x, y, z, c, re, im, mag, deg);
				first = 0;
			}
		}
		printf("]");
		fclose(nf.fp);
	}
	if( optFar ){
		// far-field export: zenith 0..180 step 15, azimuth 0..315 step 45
		FARFILE ff;
		memset(&ff, 0, sizeof(ff));
		ff.fp = tmpfile();
		ff.Z = 0; ff.ZW = 15; ff.ZN = 13;
		ff.A = 0; ff.AW = 45; ff.AN = 8;
		exeenv.pFarFile = &ff;
		SaveFar();
		exeenv.pFarFile = NULL;
		printf(",\"far\":[");
		rewind(ff.fp);
		int first = 1;
		while( fgets(line, sizeof(line), ff.fp) != NULL ){
			double v[5];
			if( sscanf(line, "%lf,%lf,%lf,%lf,%lf", &v[0], &v[1], &v[2], &v[3], &v[4]) == 5 ){
				printf("%s[%.17g,%.17g,%.17g,%.17g,%.17g]", first ? "" : ",", v[0], v[1], v[2], v[3], v[4]);
				first = 0;
			}
		}
		printf("]");
		fclose(ff.fp);
	}
	printf("}\n");
	DeleteNEC();
	return 0;
}
