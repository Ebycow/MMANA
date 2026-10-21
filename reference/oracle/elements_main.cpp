// MMANA element-editing oracle.
//
// Runs the ORIGINAL CWGroup / CWGroupList (generated/cwgroup_extract.cpp,
// unmodified) on the wires of a .MAA file through a script of edits and prints
// the element table and the resulting wires as JSON. The edit rules below are
// adapted from TGrpWireDlg (GrpWire.cpp: GridSetEditText, GridNewLine, the
// popup-menu handlers, AllRoundUp and IsSameElement) with the VCL grid
// replaced by explicit operations.
//
// usage: elements <file.maa> <script>
//
// Script: one operation per line; '#' starts a comment.
//   width|widthall|height|heightall|round|roundall|radius|interval|position <row> <value>
//   seg <row> <code>     kind <row> <name>     insert <row>     delete <row>
//   roundup <digits> [l] [all]      (l: wavelength units, all: scale all coordinates)
// <row>:   a number, new (= element count), last, base, or * (every current row)
// <value>: a number, *f (current value times f) or +d (current value plus d)

// Reuse the .MAA loader of the calculation oracle (InitEnv, InitAntDef, LoadAntStrings).
#define main oracle_calc_main
#include "oracle_main.cpp"
#undef main

// CWGroup::AnaLoop writes one WREF past the end of its list when no wire
// continues the loop. Padding array allocations keeps that write inside the
// block; only this binary is affected, not the calculation oracle.
void *operator new[](size_t n)
{
	void *p = malloc(n + 256);
	if( p == NULL ) abort();
	return p;
}
void operator delete[](void *p) noexcept { free(p); }
void operator delete[](void *p, size_t) noexcept { free(p); }

static CWGroupList *WList;

// --- GrpWire.cpp TGrpWireDlg::GridNewLine -----------------------------------
static void GridNewLine(int Row, int type = cwHDP)
{
	if( (Row >= WList->gmax) && (Row < CWMAX) ){
		WList->CreateWire(Row, type);
	}
}

// --- GrpWire.cpp TGrpWireDlg::GridSetEditText (values already in metres) ----
static int SetInterval(int Row, double d)
{
	if( d >= 0.0 ){
		if( WList->BaseW == Row ) return 0;
		GridNewLine(Row);
		WList->Intval[Row] = d;
		return 1;
	}
	return 0;
}
static int SetPosition(int Row, double d)
{
	if( d >= 0.0 ){
		if( !Row ) return 0;
		GridNewLine(Row);
		WList->SetXPosition(Row, d);
		return 1;
	}
	return 0;
}
static int SetWidth(int Row, double d, int all)
{
	if( d >= 0.001 ){
		GridNewLine(Row, cwHDP);
		WList->List[Row].SetWidth(all, d);
		return 1;
	}
	return 0;
}
static int SetHeight(int Row, double d, int all)
{
	if( d >= 0.001 ){
		GridNewLine(Row, cwVDP);
		WList->List[Row].SetHeight(all, d);
		return 1;
	}
	return 0;
}
static int SetRound(int Row, double d, int all)
{
	if( d >= 0.001 ){
		GridNewLine(Row, cwVSQ);
		WList->List[Row].SetRound(all, d);
		return 1;
	}
	return 0;
}
static int SetR(int Row, double d)
{
	if( WList->List[Row].EnbR ){
		GridNewLine(Row);
		WList->List[Row].SetR(d);
		return 1;
	}
	return 0;
}
static int SetSeg(int Row, int s)
{
	if( WList->List[Row].EnbSeg ){
		GridNewLine(Row);
		WList->List[Row].SetSeg(s);
		return 1;
	}
	return 0;
}

// --- popup menu: KS1Click.. (SetNewType), K3Click (insert), K2Click (delete) --
static int SetKind(int Row, int type)
{
	int r = (Row < WList->gmax) ? (WList->List[Row].Type != type) : (Row == WList->gmax);
	WList->SetNewType(Row, type);
	return r;
}
static int Insert(int Row)
{
	if( (Row <= WList->gmax) && (WList->gmax < CWMAX) ){
		WList->InsertItem(Row);
		return 1;
	}
	return 0;
}
static int Delete(int Row)
{
	// the menu item is disabled on the base row
	if( (Row < WList->gmax) && (Row != WList->BaseW) ){
		WList->DeleteItem(Row);
		return 1;
	}
	return 0;
}

// ComLib.cpp GetRmdVal / SetRmdVal (MmSel is set only for "roundup ... mm")
double GetRmdVal(double d)
{
	if( exeenv.RmdSel ){
		d = d / exeenv.rmd;
	}
	else if( exeenv.MmSel ){
		d = d * 1000.0;
	}
	return d;
}
double SetRmdVal(double d)
{
	if( exeenv.RmdSel ){
		d = d * exeenv.rmd;
	}
	else if( exeenv.MmSel ){
		d = d / 1000.0;
	}
	return d;
}

// --- GrpWire.cpp TGrpWireDlg::AllRoundUp (without the confirmation) ---------
static void AllRoundUp(double sc, int EleWidthAll)
{
	CWGroup *gp = WList->List;
	int i;
	double R;
	for( i = 0; i < WList->gmax; i++, gp++ ){
		if( gp->EnbXw() ){
			if( gp->GetWidth() ){
				gp->SetWidth(EleWidthAll, SetRmdVal(RoundUp(GetRmdVal(gp->GetWidth()), sc)));
			}
			if( gp->GetHeight() ){
				gp->SetHeight(EleWidthAll, SetRmdVal(RoundUp(GetRmdVal(gp->GetHeight()), sc)));
			}
			if( gp->GetRound() ){
				gp->SetRound(EleWidthAll, SetRmdVal(RoundUp(GetRmdVal(gp->GetRound()), sc)));
			}
		}
		else if( gp->GetRound() ){
			gp->SetRound(EleWidthAll, SetRmdVal(RoundUp(GetRmdVal(gp->GetRound()), sc)));
		}
		if( WList->GetXPosition(i) ){
			WList->SetXPosition(i, SetRmdVal(RoundUp(GetRmdVal(WList->GetXPosition(i)), sc)));
		}
		if( gp->EnbR && (gp->GetR() > 0.0) ){
			if( exeenv.RmdSel ){
				R = SetRmdVal(RoundUp(GetRmdVal(gp->GetR()), sc));
				if( !R ) R = SetRmdVal(1/sc);
			}
			else {
				R = RoundUp(gp->GetR(), sc * 1000.0);
				if( !R ) R = 1/sc*1000.0;
			}
			gp->SetR(R);
		}
	}
}

// --- GrpWire.cpp TGrpWireDlg::IsSameElement ---------------------------------
static int IsSameElement(int i, int j)
{
	if( WList->List[i].bcnt != WList->List[j].bcnt ) return FALSE;
	WREF	*rp, *tp;

	int k;
	rp = WList->List[i].pB;
	tp = WList->List[j].pB;
	for( k = 0; k < WList->List[i].bcnt; k++, rp++, tp++ ){
		if( rp->wp->X1 != tp->wp->X1 ) return FALSE;
		if( rp->wp->Y1 != tp->wp->Y1 ) return FALSE;
		if( rp->wp->Z1 != tp->wp->Z1 ) return FALSE;
		if( rp->wp->X2 != tp->wp->X2 ) return FALSE;
		if( rp->wp->Y2 != tp->wp->Y2 ) return FALSE;
		if( rp->wp->Z2 != tp->wp->Z2 ) return FALSE;
	}
	return TRUE;
}
static int FindSameElement(int &a, int &b)
{
	int i, j;

	for( i = WList->BaseW; i > 0; i-- ){
		for( j = i - 1; (j >= 0) && (!WList->Intval[j]); j-- ){
			if( IsSameElement(i, j) == TRUE ){ a = i; b = j; return TRUE; }
		}
	}
	for( i = WList->BaseW; i < (WList->gmax - 1); i++ ){
		for( j = i + 1; (j < WList->gmax) && (!WList->Intval[j]); j++ ){
			if( IsSameElement(i, j) == TRUE ){ a = i; b = j; return TRUE; }
		}
	}
	return FALSE;
}

// --- script ------------------------------------------------------------------
static int KindByName(const char *s)
{
	static const struct { const char *name; int type; } kinds[] = {
		{"hdp", cwHDP}, {"vdp", cwVDP}, {"vsq", cwVSQ}, {"vdm", cwVDM}, {"vita", cwVITA},
		{"vta", cwVTA}, {"vpie", cwVPIE}, {"vsq2", cwVSQ2}, {"vta2", cwVTA2}, {"hsq", cwHSQ},
		{"hdm", cwHDM}, {"hta", cwHTA}, {"hita", cwHITA},
	};
	for( size_t i = 0; i < sizeof(kinds)/sizeof(kinds[0]); i++ ){
		if( !strcmp(s, kinds[i].name) ) return kinds[i].type;
	}
	fprintf(stderr, "unknown kind %s\n", s);
	exit(2);
}

static double Current(const char *op, int Row)
{
	if( Row >= WList->gmax ) return 0.0;
	CWGroup *gp = &WList->List[Row];
	if( !strncmp(op, "width", 5) ) return gp->GetWidth();
	if( !strncmp(op, "height", 6) ) return gp->GetHeight();
	if( !strncmp(op, "round", 5) ) return gp->GetRound();
	if( !strcmp(op, "radius") ) return gp->GetR();
	if( !strcmp(op, "interval") ) return WList->Intval[Row];
	if( !strcmp(op, "position") ) return WList->GetXPosition(Row);
	return 0.0;
}

static int RunOp(const char *op, int Row, const char *arg)
{
	if( !strcmp(op, "kind") ) return SetKind(Row, KindByName(arg));
	if( !strcmp(op, "insert") ) return Insert(Row);
	if( !strcmp(op, "delete") ) return Delete(Row);
	if( !strcmp(op, "seg") ) return SetSeg(Row, atoi(arg));
	double d;
	if( arg[0] == '*' ) d = Current(op, Row) * atof(arg + 1);
	else if( arg[0] == '+' ) d = Current(op, Row) + atof(arg + 1);
	else d = atof(arg);
	if( !strcmp(op, "width") ) return SetWidth(Row, d, 0);
	if( !strcmp(op, "widthall") ) return SetWidth(Row, d, 1);
	if( !strcmp(op, "height") ) return SetHeight(Row, d, 0);
	if( !strcmp(op, "heightall") ) return SetHeight(Row, d, 1);
	if( !strcmp(op, "round") ) return SetRound(Row, d, 0);
	if( !strcmp(op, "roundall") ) return SetRound(Row, d, 1);
	if( !strcmp(op, "radius") ) return SetR(Row, d);
	if( !strcmp(op, "interval") ) return SetInterval(Row, d);
	if( !strcmp(op, "position") ) return SetPosition(Row, d);
	fprintf(stderr, "unknown operation %s\n", op);
	exit(2);
}

static void PutD(const char *key, double d)
{
	printf("\"%s\":%.17g", key, d);
}

int main(int argc, char **argv)
{
	if( argc < 3 ){
		fprintf(stderr, "usage: elements <file.maa> <script>\n");
		return 2;
	}
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
	InitAntDef();
	LoadAntStrings(in);
	free(in);
	exeenv.rmd = ant.fq ? 299.8/ant.fq : 0.0;		// TMainWnd::SetAntDef

	// TGrpWireDlg::Execute
	WList = new CWGroupList;
	WList->SetWire(&ant);

	FILE *sp = fopen(argv[2], "rt");
	if( sp == NULL ){
		fprintf(stderr, "cannot open %s\n", argv[2]);
		return 2;
	}
	printf("{");
	PutD("rmd", exeenv.rmd);
	printf(",\"applied\":[");
	int nop = 0;
	while( fgets(line, sizeof(line), sp) != NULL ){
		char *c = strchr(line, '#');
		if( c != NULL ) *c = 0;
		char op[32], row[32], a1[32], a2[32], a3[32];
		a1[0] = a2[0] = a3[0] = 0;
		int n = sscanf(line, "%31s %31s %31s %31s %31s", op, row, a1, a2, a3);
		if( n < 1 ) continue;
		if( !strcmp(op, "roundup") ){
			int digits = atoi(row);
			exeenv.RmdSel = (!strcmp(a1, "l") || !strcmp(a2, "l") || !strcmp(a3, "l")) ? 1 : 0;
			exeenv.MmSel = (!strcmp(a1, "mm") || !strcmp(a2, "mm") || !strcmp(a3, "mm")) ? 1 : 0;
			int all = (!strcmp(a1, "all") || !strcmp(a2, "all") || !strcmp(a3, "all")) ? 1 : 0;
			AllRoundUp(pow(10.0, digits), all);
			exeenv.RmdSel = 0;
			exeenv.MmSel = 0;
			printf("%s1", nop++ ? "," : "");
			continue;
		}
		int first, last;
		if( !strcmp(row, "*") ){ first = 0; last = WList->gmax - 1; }
		else if( !strcmp(row, "new") ) first = last = WList->gmax;
		else if( !strcmp(row, "last") ) first = last = WList->gmax - 1;
		else if( !strcmp(row, "base") ) first = last = WList->BaseW;
		else first = last = atoi(row);
		for( int r = first; r <= last; r++ ){
			int ok = (r >= 0) && (r <= WList->gmax) ? RunOp(op, r, a1) : 0;
			printf("%s%d", nop++ ? "," : "", ok);
		}
	}
	fclose(sp);
	printf("]");

	int a, b;
	if( FindSameElement(a, b) ) printf(",\"same\":[%d,%d]", a, b);
	else printf(",\"same\":null");
	printf(",\"base\":%d,\"rows\":[", WList->BaseW);
	for( int i = 0; i < WList->gmax; i++ ){
		CWGroup *gp = &WList->List[i];
		printf("%s{\"type\":%d,", i ? "," : "", gp->Type);
		PutD("interval", WList->Intval[i]); printf(",");
		PutD("position", WList->GetXPosition(i)); printf(",");
		PutD("width", gp->GetWidth()); printf(",");
		PutD("height", gp->GetHeight()); printf(",");
		PutD("round", gp->GetRound()); printf(",");
		PutD("r", gp->GetR());
		printf(",\"xw\":%d,\"enbR\":%d,\"enbSeg\":%d,\"seg\":%d,\"wires\":%d}",
			gp->EnbXw(), gp->EnbR, gp->EnbSeg, gp->GetSEG(), gp->bcnt);
	}
	printf("],\"wires\":[");
	WList->Update();
	for( int i = 0; i < ant.wmax; i++ ){
		WDEF *wp = &ant.wdef[i];
		printf("%s[%.17g,%.17g,%.17g,%.17g,%.17g,%.17g,%.17g,%d]", i ? "," : "",
			wp->X1, wp->Y1, wp->Z1, wp->X2, wp->Y2, wp->Z2, wp->R, wp->SEG);
	}
	printf("]}\n");
	return 0;
}
