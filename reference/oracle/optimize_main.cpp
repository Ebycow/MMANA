// MMANA optimisation oracle.
//
// Runs the ORIGINAL optimiser (generated/cacal_extract.cpp: CACal and
// CACal::Exec; generated/acalbox_extract.cpp: the variable helpers of the
// setup dialog; both unmodified) on a .MAA file as a case file describes and
// prints every calculation, the remembered trials and the final antenna as
// JSON. One trial (TMainWnd::ACalExecute), the start of a run
// (TMainWnd::ACalBtnClick, TACalDlg::Execute / OKBtnClick) and the dialog's
// simple handlers (GridNewLine, SetType, K27Click, the cell edits) are adapted
// below with the VCL replaced by plain variables.
//
// usage: optimize <case>
//
// Case file: one setting per line; '#' starts a comment. Indices are 0-based.
//   file <name.MAA>                 (relative to testdata/maa)
//   freq <MHz>  ground <0|1|2>  height <m>
//   weights <gain> <fb> <jx> <swr> <elv> <match> <cur>    (0-100 each)
//   sweep <0|1>  pitchabs <0|1>  coarse <0|1>  emphasize <0|1>
//   maxgain <dBi>  maxfb <dB>  minjx <ohm>  minswr <swr>
//   match <0|1|2> [<r|*> <x|*>]     current <pulse> <0 max|1 min>
//   band <MHz> <pulse> <deg> <volt> [<pulse> <deg> <volt> ...]
//   limit <calculations>
//   set <var> <wire|ele|load|height|freq|wirep|feed|stack> <pos> <sub>
//   insert <var>  delete <var>  clear  linksame <var>  linkele
//   allele <intervals 0|1> <scaleall 0|1> [<hw> <vw> <xw> <hc> <vc> <xc>]
//   var <var> [pitch=<d>] [min=<d>|min=@<var>] [max=<d>|max=@<var>]   (@: the value of another variable, MMANA's #n)
//             [link=none|same:<v>|neg:<v>|offset:<v>:<c>|negoffset:<v>:<c>|scaled:<v>:<c>|divided:<v>:<c>]

// Reuse the .MAA loader and the stacking of the calculation oracle.
#define main oracle_calc_main
#include "oracle_main.cpp"
#undef main
#include "ACalBox.h"

#include <string>

// CWGroup::AnaLoop writes one WREF past the end of its list when no wire
// continues the loop (see elements_main.cpp).
void *operator new[](size_t n)
{
	void *p = malloc(n + 256);
	if( p == NULL ) abort();
	return p;
}
void operator delete[](void *p) noexcept { free(p); }
void operator delete[](void *p, size_t) noexcept { free(p); }

ANTDEF *OracleAnt(void) { return &ant; }

CACal *pACal;
static int CalcLimit = 400;
static int CalcCount = 0;
static int EleAnswer[6] = { 1, 1, 1, 0, 0, 0 };
static std::string Calcs;
static std::string Errors;

void ErrorMB(LPCSTR fmt, ...)
{
	char bf[512];
	va_list pp;
	va_start(pp, fmt);
	vsnprintf(bf, sizeof(bf), fmt, pp);
	va_end(pp);
	if( !Errors.empty() ) Errors += "; ";
	Errors += bf;
}
// The dialogs' questions ("連携先の変数の値に強制設定されます。よろしいですか？", ...) are answered yes.
int YesNoMB(LPCSTR, ...) { return IDYES; }
void LogPrint(LPCSTR, ...) {}
LPCSTR StrDbl1(double d)
{
	static char bf[64];
	snprintf(bf, sizeof(bf), "%.1f", d);
	return bf;
}

int TACalEleBox::Execute(int &HW, int &VW, int &XW, int &HC, int &VC, int &XC)
{
	HW = HW && EleAnswer[0];
	VW = VW && EleAnswer[1];
	XW = XW && EleAnswer[2];
	HC = EleAnswer[3];
	VC = EleAnswer[4];
	XC = EleAnswer[5];
	return TRUE;
}

static void PutNum(std::string &s, double d)
{
	char bf[64];
	snprintf(bf, sizeof(bf), "%.17g", d);
	s += bf;
}

// --- Main.cpp TMainWnd::SetStackAnt ---------------------------------------
static int SetStackAnt(void)
{
	if( (ant.StackH == 1)&&(ant.StackV == 1) ){
		pCalAnt = &ant;
	}
	else {
		pCalAnt = &calAnt;
		Stack();
	}
	return InitAntData(pCalAnt);
}

// --- Main.cpp TMainWnd::ACalExecute (wires below ground: MMANA asks whether
// to stop; the oracle goes on). Records the calculation; DrawPtnACalInfo,
// which Exec calls after scoring it, adds the current and the score.
int TMainWndShim::ACalExecute(void)
{
	ant.Flag = 0;
	exeenv.CalcLog = 0;
	if( SetStackAnt() == TRUE ){
		res.GetResP()->HEIGHT = env.antheight;
		if( pACal->DegUnit ){
			CalCurrentDR();			// ２°分解能
		}
		else {
			CalCurrent();			// １°分解能
		}
		res.CalcF = 1;
		RESL *rp = res.GetResP();
		Calcs += Calcs.empty() ? "[[" : ",[[";
		for( int i = 0; i < pACal->amax; i++ ){
			if( i ) Calcs += ",";
			PutNum(Calcs, pACal->aval[i].DC);
		}
		Calcs += "]";
		double v[] = { ant.cfq, rp->R, rp->JX, rp->SWR, rp->GAIN, rp->FB, rp->ELV };
		for( double d : v ){
			Calcs += ",";
			PutNum(Calcs, d);
		}
		if( ++CalcCount >= CalcLimit ) exeenv.CalcAbort = 1;
		return TRUE;
	}
	return FALSE;
}

void TMainWndShim::DrawPtnACalInfo(void)
{
	Calcs += ",";
	PutNum(Calcs, pACal->atrg.CurCur);
	Calcs += ",";
	PutNum(Calcs, pACal->atrg.V);
	Calcs += "]";
}

// --- ACalBox.cpp TACalDlg::GridNewLine, SetType / SetSubPos / the 位置 cell
static void SetVariable(int Row, int type, int pos, int sub)
{
	if( (Row >= pACal->amax) && (Row < AVMAX) ){
		AVAL *ap = &pACal->aval[pACal->amax];
		pACal->amax++;
		ap->With = -1;
	}
	pACal->aval[Row].Type = type;
	pACal->aval[Row].Pos = pos;
	pACal->aval[Row].SubPos = sub;
	pACal->AdjSetup(Row);
}

static int TypeByName(const char *s)
{
	const char *names[] = { "wire", "ele", "load", "height", "freq", "wirep", "feed", "stack" };
	for( int i = 0; i < 8; i++ ){
		if( !strcmp(s, names[i]) ) return i;
	}
	fprintf(stderr, "unknown variable type %s\n", s);
	exit(2);
}

// --- the 連携 / ﾋﾟｯﾁ / Min / Max cells, given as values
static void SetField(AVAL *ap, char *kv)
{
	char *v = strchr(kv, '=');
	if( v == NULL ) return;
	*v++ = 0;
	if( !strcmp(kv, "pitch") ){
		ap->DW = atof(v);
	}
	else if( !strcmp(kv, "min") || !strcmp(kv, "max") ){
		USHORT &flag = (kv[1] == 'i') ? ap->Min : ap->Max;
		double &d = (kv[1] == 'i') ? ap->DMIN : ap->DMAX;
		if( *v == '@' ){
			flag = USHORT(atoi(v + 1) | 0x8000);
		}
		else {
			flag = 0;
			d = atof(v);
		}
	}
	else if( !strcmp(kv, "link") ){
		char *k = strchr(v, ':');
		int n = k ? atoi(k + 1) : 0;
		char *c = k ? strchr(k + 1, ':') : NULL;
		double dc = c ? atof(c + 1) : 0.0;
		if( !strncmp(v, "none", 4) ) ap->With = -1;
		else if( !strncmp(v, "same", 4) ) ap->With = n;
		else if( !strncmp(v, "neg:", 4) ) ap->With = -n - 2;
		else if( !strncmp(v, "offset", 6) ){ ap->With = n | 0x00010000; ap->DW = dc; }
		else if( !strncmp(v, "negoffset", 9) ){ ap->With = n | 0x00020000; ap->DW = dc; }
		else if( !strncmp(v, "scaled", 6) ){ ap->With = n | 0x00040000; ap->DW = dc; }
		else if( !strncmp(v, "divided", 7) ){ ap->With = n | 0x00080000; ap->DW = dc; }
	}
}

static void PutVariables(void)
{
	printf("\"variables\":[");
	for( int i = 0; i < pACal->amax; i++ ){
		AVAL *ap = &pACal->aval[i];
		printf("%s[%d,%d,%d,%d,%.17g,%.17g,%.17g,%d,%d]", i ? "," : "", ap->Type, ap->Pos, ap->SubPos, ap->With,
			ap->DW, ap->DMIN, ap->DMAX, ap->Min, ap->Max);
	}
	printf("]");
}

int main(int argc, char **argv)
{
	if( argc < 2 ){
		fprintf(stderr, "usage: optimize <case>\n");
		return 2;
	}
	FILE *cf = fopen(argv[1], "rt");
	if( cf == NULL ){
		fprintf(stderr, "cannot open %s\n", argv[1]);
		return 2;
	}
	char dir[1024];
	strncpy(dir, argv[1], sizeof(dir) - 1);
	dir[sizeof(dir) - 1] = 0;
	char *slash = strrchr(dir, '/');
	if( slash ) *slash = 0; else strcpy(dir, ".");

	char line[1024];
	char *lines[512];
	int nlines = 0;
	while( (nlines < 512) && (fgets(line, sizeof(line), cf) != NULL) ){
		char *h = strchr(line, '#');
		if( h ) *h = 0;
		for( char *s = line; *s; s++ ){
			if( (*s == '\r') || (*s == '\n') ){ *s = 0; break; }
		}
		lines[nlines++] = strdup(line);
	}
	fclose(cf);

	// the antenna: TMainWnd::LoadAntFile as in the calculation oracle
	char path[2048] = "";
	double optFreq = 0.0, optHeight = -1.0;
	int optGround = -1;
	for( int i = 0; i < nlines; i++ ){
		char key[64], arg[1024];
		if( sscanf(lines[i], "%63s %1023s", key, arg) != 2 ) continue;
		if( !strcmp(key, "file") ) snprintf(path, sizeof(path), "%s/../../maa/%s", dir, arg);
		else if( !strcmp(key, "freq") ) optFreq = atof(arg);
		else if( !strcmp(key, "ground") ) optGround = atoi(arg);
		else if( !strcmp(key, "height") ) optHeight = atof(arg);
	}
	FILE *fp = fopen(path, "rt");
	if( fp == NULL ){
		fprintf(stderr, "cannot open %s\n", path);
		return 2;
	}
	size_t cap = 1 << 20, len = 0;
	char *in = (char *)malloc(cap);
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
	char bf[64];
	double d;
	if( ant.fq ) snprintf(bf, sizeof(bf), "%.3lf", ant.fq); else bf[0] = 0;
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

	// --- Main.cpp TMainWnd::ACalBtnClick
	if( ant.cfq == 0.0 ) ant.cfq = ant.fq;
	pACal = new CACal;
	int weights[7] = { int(pACal->atrg.TG), int(pACal->atrg.TFB), int(pACal->atrg.TJX), int(pACal->atrg.TSWR),
		int(pACal->atrg.TELV), int(pACal->atrg.TM), int(pACal->atrg.TI) };
	for( int i = 0; i < nlines; i++ ){
		char key[64];
		char a[7][256];
		int n = sscanf(lines[i], "%63s %255s %255s %255s %255s %255s %255s %255s", key, a[0], a[1], a[2], a[3], a[4], a[5], a[6]);
		if( n < 2 ) continue;
		ATRG &at = pACal->atrg;
		if( !strcmp(key, "weights") ){
			for( int k = 0; k < 7 && k < n - 1; k++ ) weights[k] = atoi(a[k]);
		}
		else if( !strcmp(key, "sweep") ) at.TF = !atoi(a[0]);
		else if( !strcmp(key, "pitchabs") ) pACal->PitchUnit = atoi(a[0]);
		else if( !strcmp(key, "coarse") ) pACal->DegUnit = atoi(a[0]);
		else if( !strcmp(key, "emphasize") ) at.bweight = atoi(a[0]);
		else if( !strcmp(key, "maxgain") ) at.MaxGain = atof(a[0]);
		else if( !strcmp(key, "maxfb") ) at.MaxFB = atof(a[0]);
		else if( !strcmp(key, "minjx") ) at.MinJX = atof(a[0]);
		else if( !strcmp(key, "minswr") ) at.MinSWR = atof(a[0]);
		else if( !strcmp(key, "match") ){
			at.Match = atoi(a[0]);
			if( n >= 4 ){
				at.TergR = (a[1][0] == '*') ? NULLV : atof(a[1]);
				at.TergJX = (a[2][0] == '*') ? NULLV : atof(a[2]);
			}
		}
		else if( !strcmp(key, "current") ){
			ZSCopy(at.IPLUS, a[0], 8);
			if( n >= 3 ) at.IType = atoi(a[1]);
		}
		else if( !strcmp(key, "band") && (at.bmax < BMAX) ){
			BDEF *bp = &at.bdef[at.bmax++];
			memset(bp, 0, sizeof(BDEF));
			bp->fq = atof(a[0]);
			char *p = lines[i];
			while( *p == ' ' || *p == '\t' ) p++;
			p += 4;
			double fq;
			int used;
			sscanf(p, "%lf%n", &fq, &used);
			p += used;
			for( int c = 0; c < 4; c++ ){
				char pulse[64];
				double deg, ev;
				if( sscanf(p, "%63s %lf %lf%n", pulse, &deg, &ev, &used) != 3 ) break;
				p += used;
				ZSCopy(bp->cdef[c].PLUS, pulse, 8);
				bp->cdef[c].DEG = deg;
				bp->cdef[c].EV = ev;
			}
		}
		else if( !strcmp(key, "limit") ) CalcLimit = atoi(a[0]);
	}
	if( ant.cmax && (pACal->atrg.bmax <= 1) ){
		memset(pACal->atrg.bdef, 0, sizeof(pACal->atrg.bdef));
		pACal->atrg.bmax = 1;
		pACal->atrg.bdef[0].fq = ant.cfq ? ant.cfq : ant.fq;
		for( int i = 0; (i < ant.cmax) && (i < 4); i++ ){
			memcpy(&pACal->atrg.bdef[0].cdef[i], &ant.cdef[i], sizeof(CDEF));
		}
	}
	pACal->Init(&ant, &env);

	// --- the setup dialog
	TACalDlg dlg(pACal);
	if( pACal->AP->cmax > 4 ) pACal->atrg.bmax = 1;
	for( int i = 0; i < nlines; i++ ){
		char key[64];
		char a[8][256];
		int n = sscanf(lines[i], "%63s %255s %255s %255s %255s %255s %255s %255s %255s", key, a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7]);
		if( n < 1 ) continue;
		if( !strcmp(key, "set") && n >= 5 ){
			SetVariable(atoi(a[0]), TypeByName(a[1]), atoi(a[2]), atoi(a[3]));
		}
		else if( !strcmp(key, "insert") ){
			dlg.Grid->Row = atoi(a[0]) + 1;
			dlg.K28Click(NULL);
		}
		else if( !strcmp(key, "delete") ){
			dlg.Grid->Row = atoi(a[0]) + 1;
			dlg.DelBtnClick(NULL);
		}
		else if( !strcmp(key, "clear") ){
			pACal->amax = 0;			// K27Click
			memset(&pACal->aval, 0, sizeof(pACal->aval));
		}
		else if( !strcmp(key, "linksame") ){
			dlg.Grid->Row = atoi(a[0]) + 1;
			dlg.K11Click(NULL);
		}
		else if( !strcmp(key, "linkele") ){
			dlg.K34Click(NULL);
		}
		else if( !strcmp(key, "allele") && n >= 3 ){
			exeenv.IntPos = atoi(a[0]);
			exeenv.EleWidthAll = atoi(a[1]);
			for( int k = 0; k < 6 && k + 2 < n - 1; k++ ) EleAnswer[k] = atoi(a[k + 2]);
			dlg.AllEleClick(NULL);
		}
		else if( !strcmp(key, "var") && n >= 2 ){
			AVAL *ap = &pACal->aval[atoi(a[0])];
			for( int k = 1; k < n - 1; k++ ) SetField(ap, a[k]);
		}
	}
	pACal->SetTradeOff(weights[0], weights[1], weights[2], weights[3], weights[4], weights[5], weights[6]);

	printf("{");
	PutVariables();
	// OKBtnClick
	if( (pACal->Check() == FALSE) || (pACal->InitPara() == FALSE) ){
		printf(",\"error\":\"");
		for( const char *s = Errors.c_str(); *s; s++ ){
			unsigned char c = (unsigned char)*s;
			if( (c == '"') || (c == '\\') ) printf("\\%c", c);
			else if( c < 0x20 ) printf("?");
			else putchar(c);
		}
		printf("\"}\n");
		return 0;
	}

	// --- ACalBtnClick: the run
	res.IncResP();
	res.GetResP()->TYPE = env.type;
	res.GetResP()->HEIGHT = env.antheight;
	res.GetResP()->FBR = env.fbr;
	exeenv.CalcF = 1;
	exeenv.CalcDisp = 1;
	exeenv.CalcAbort = 0;
	int r = pACal->Execute();

	printf(",\"result\":%d,\"vm\":%.17g,\"ad\":[", r, pACal->atrg.VM);
	for( int i = 0; i < pACal->amax; i++ ) printf("%s%.17g", i ? "," : "", pACal->atrg.AD[i]);
	printf("],\"calcs\":[%s]", Calcs.c_str());
	printf(",\"trials\":[");
	for( int i = 0; i < pACal->GetMV(); i++ ){
		MVAL m;
		pACal->ReadMV(&m, i);
		printf("%s{\"v\":%.17g,\"values\":[", i ? "," : "", m.V);
		for( int k = 0; k < pACal->amax; k++ ) printf("%s%.17g", k ? "," : "", m.AD[k]);
		printf("],\"r\":%.9g,\"jx\":%.9g,\"swr\":%.9g,\"gain\":%.9g,\"fb\":%.9g,\"elv\":%.9g,\"h\":[",
			m.CRR, m.CJX, m.CSWR, m.CG, m.CFB, m.CELV);
		for( int k = 0; k < 180; k++ ) printf("%s%d", k ? "," : "", m.PtnH[k]);
		printf("],\"v2\":[");
		for( int k = 0; k <= 90; k++ ) printf("%s%d", k ? "," : "", m.PtnV[k]);
		printf("]}");
	}
	printf("],\"antenna\":{\"wires\":[");
	for( int i = 0; i < ant.wmax; i++ ){
		WDEF *wp = &ant.wdef[i];
		printf("%s[%.17g,%.17g,%.17g,%.17g,%.17g,%.17g,%.17g]", i ? "," : "", wp->X1, wp->Y1, wp->Z1, wp->X2, wp->Y2, wp->Z2, wp->R);
	}
	printf("],\"loads\":[");
	for( int i = 0; i < ant.lmax; i++ ) printf("%s[%.17g,%.17g]", i ? "," : "", ant.ldef[i].A[0], ant.ldef[i].B[0]);
	printf("],\"sources\":[");
	for( int i = 0; i < ant.cmax; i++ ) printf("%s[%.17g,%.17g]", i ? "," : "", ant.cdef[i].DEG, ant.cdef[i].EV);
	printf("],\"height\":%.17g,\"freq\":%.17g,\"stack\":[%.17g,%.17g]}}\n", env.antheight, ant.cfq, ant.StackHW, ant.StackVW);
	return 0;
}
