// MMANA calculator oracle.
//
// Runs the ORIGINAL calculations of the calculator dialog (TOptDlgBox in
// OptDlg.cpp: generated/optdlg_extract.cpp, unmodified) on the cases of a
// text file and prints one JSON object per case. The unit conversions around
// each call are those of the dialog's edit handlers (TEditLChange,
// AEditWChange, CEditLCChange, ...) and of SetValueItem, which shows the
// results; the VCL edit boxes are replaced by the arguments of a case.
//
// usage: tools <cases.txt>
//
// One case per line; '#' starts a comment.
//   tank x <MHz> <X>  |  tank l <MHz> <uH>  |  tank c <MHz> <pF>  |  tank lc <uH> <pF>
//   coil l <N> <D cm> <wire mm> <space mm>  |  coil n <uH> <D> <wire> <space>  |  coil d <uH> <N> <wire> <space>
//   match <MHz> <Zo> <R> <X> <0 shunt C | 1 series C>
//   stub <MHz> <cable> <vf> <Zo> <open|short> x <X> | l <uH> | c <pF> | len <cm>   (vf, Zo used when cable = 0)
//   line zi <Zo> <len wl> <loss dB> <RL> <XL>  |  line zl <Zo> <len wl> <loss dB> <Ri> <Xi>
//   qm <RL> <XL> <Ri> <Zo1> <L1 wl> <Zo2> <L2 wl>
//   qmtune <RL> <XL> <Ri> <Zo1> <Zo2>
//   sm <MHz> <Zi> <R> <X> <Zo> <vf>
//   hairpin <R> <Z>
//   bwmatch <RO> <JXO> <f0 MHz> <R0> <X0> <f MHz> <R> <X>
//     the 周波数特性 dialog's matching (TBwDispDlg in BwDisp.cpp:
//     generated/bwdisp_extract.cpp): the antenna, R0 + jX0 at f0, matched to
//     RO; the SWR (CalcSWR, BwMatch = 1) and impedance (CalcZ, BwMatch = 2)
//     at f of an antenna that is R + jX there

// Reuse the helpers of the calculation oracle (StrDbl, WarningMB, ...).
#define main oracle_calc_main
#include "oracle_main.cpp"
#undef main

#include "OptDlg.h"
#include "BwDisp.h"

static void num(const char *key, double d, int comma = 1)
{
	if( comma ) printf(",");
	if( (d != d) || (d > DBL_MAX) || (d < -DBL_MAX) ) printf("\"%s\":null", key);
	else printf("\"%s\":%.17g", key, d);
}

// A value the dialog leaves blank (NULLV).
static void opt(const char *key, double d)
{
	if( d == NULLV ) printf(",\"%s\":null", key);
	else num(key, d);
}

static void kind(const char *key, char c)
{
	printf(",\"%s\":\"%c\"", key, c);
}

static int Tank(TOptDlgBox &b, int argc, char **argv)
{
	if( argc < 3 ) return 0;
	const char *m = argv[0];
	double p = atof(argv[1]), q = atof(argv[2]);
	b.Freq = NULLV; b.X = NULLV; b.L = NULLV; b.C = NULLV;
	if( !strcmp(m, "x") ){ b.Freq = p * 1e6; b.X = ABS(q); b.CalcTank(0); }
	else if( !strcmp(m, "l") ){ b.Freq = p * 1e6; b.L = q * 1e-6; b.CalcTank(1); }
	else if( !strcmp(m, "c") ){ b.Freq = p * 1e6; b.C = q * 1e-12; b.CalcTank(2); }
	else if( !strcmp(m, "lc") ){ b.L = p * 1e-6; b.C = q * 1e-12; b.CalcTank(3); }
	else return 0;
	// SetValueItem
	printf("\"tool\":\"tank\"");
	if( b.Freq == NULLV ) printf(",\"freq\":null"); else num("freq", b.Freq / 1e6);
	opt("x", b.X);
	if( b.L == NULLV ) printf(",\"l\":null"); else num("l", b.L * 1e6);
	if( b.C == NULLV ) printf(",\"c\":null"); else num("c", b.C * 1e12);
	if( b.Freq != NULLV ){
		double rmd = 299.8e6/b.Freq;
		num("wl", rmd); num("wl2", rmd/2); num("wl4", rmd/4); num("wl58", rmd*5/8);
	}
	return 1;
}

static int Coil(TOptDlgBox &b, int argc, char **argv)
{
	if( argc < 5 ) return 0;
	const char *m = argv[0];
	double p = atof(argv[1]), q = atof(argv[2]);
	b.CoilW = atof(argv[3]) * 0.1;
	b.CoilS = atof(argv[4]) * 0.1;
	b.CoilL = NULLV; b.CoilN = 10; b.CoilD = 10.0;
	if( !strcmp(m, "l") ){ b.CoilN = p; b.CoilD = q; b.CalcCoil(0); }
	else if( !strcmp(m, "n") ){ b.CoilL = p * 1e-6; b.CoilD = q; b.CalcCoil(1); }
	else if( !strcmp(m, "d") ){ b.CoilL = p * 1e-6; b.CoilN = q; b.CalcCoil(2); }
	else return 0;
	printf("\"tool\":\"coil\"");
	if( b.CoilL == NULLV ) printf(",\"l\":null"); else num("l", b.CoilL * 1e6);
	num("n", b.CoilN);
	num("d", b.CoilD);
	num("len", (b.CoilW + b.CoilS) * b.CoilN);
	return 1;
}

static int Match(TOptDlgBox &b, int argc, char **argv)
{
	if( argc < 5 ) return 0;
	b.MF = atof(argv[0]) * 1e6;
	b.MZ = atof(argv[1]);
	b.MR = atof(argv[2]);
	b.MX = atof(argv[3]);
	b.MXSel->ItemIndex = atoi(argv[4]);
	b.MT = 0; b.MD1 = 0; b.MD2 = 0; b.MX1 = 0; b.MX2 = 0;
	b.MM1[0] = b.MM1[1] = b.MM2[0] = b.MM2[1] = 0;
	b.CalcMatch();
	// PBoxMPaint draws the shunt element when MD1 != 0 and the series one when MD2 != 0
	printf("\"tool\":\"match\"");
	printf(",\"mt\":%d", b.MT);
	if( b.MD1 ){ printf(",\"shunt\":{"); printf("\"kind\":\"%c\"", b.MM1[0]); num("value", b.MD1); num("x", b.MX1); printf("}"); }
	else printf(",\"shunt\":null");
	if( b.MD2 ){ printf(",\"series\":{"); printf("\"kind\":\"%c\"", b.MM2[0]); num("value", b.MD2); num("x", b.MX2); printf("}"); }
	else printf(",\"series\":null");
	return 1;
}

static int Stub(TOptDlgBox &b, int argc, char **argv)
{
	if( argc < 7 ) return 0;
	b.CoreFreq = atof(argv[0]) * 1e6;
	b.CoreType = atoi(argv[1]);
	if( b.CoreType ){
		b.CoreP = CorePT[b.CoreType];
		b.CoreZ = CoreZT[b.CoreType];
	}
	else {
		b.CoreP = atof(argv[2]);
		b.CoreZ = atof(argv[3]);
	}
	b.CStub->ItemIndex = strcmp(argv[4], "short") ? 0 : 1;
	const char *m = argv[5];
	double v = atof(argv[6]);
	b.CoreX = 0; b.CoreL = 0; b.CoreLC = 0; b.CoreFo = NULLV;
	if( !strcmp(m, "x") ){ b.CoreX = v; b.CalcCore(1); }					// CEditXChange
	else if( !strcmp(m, "l") ){ b.CoreX = 1; b.CoreLC = v; b.CalcCore(0); }	// CEditLCChange showing L
	else if( !strcmp(m, "c") ){ b.CoreX = -1; b.CoreLC = v; b.CalcCore(0); }	// CEditLCChange showing C
	else if( !strcmp(m, "len") ){ b.CoreL = v; b.CalcCore(3); }				// CEditLChange
	else return 0;
	printf("\"tool\":\"stub\"");
	num("p", b.CoreP);
	num("z", b.CoreZ);
	num("x", b.CoreX);
	kind("kind", (b.CoreX >= 0.0) ? 'L' : 'C');
	num("lc", b.CoreLC);
	num("len", b.CoreL);
	opt("fo", b.CoreFo);
	return 1;
}

static int Line(TOptDlgBox &b, int argc, char **argv)
{
	if( argc < 6 ) return 0;
	const char *m = argv[0];
	b.QMZo = atof(argv[1]);
	b.QML = atof(argv[2]);
	b.QMA = atof(argv[3]);
	double r = atof(argv[4]), x = atof(argv[5]);
	if( !strcmp(m, "zi") ){ b.QMRL1 = r; b.QMXL1 = x; b.CalcZi(b.QMRi1, b.QMXi1, b.QMZo, b.QML, b.QMRL1, b.QMXL1, b.QMA); }
	else if( !strcmp(m, "zl") ){ b.QMRi1 = r; b.QMXi1 = x; b.CalcZL(b.QMRL1, b.QMXL1, b.QMZo, b.QML, b.QMRi1, b.QMXi1, b.QMA); }
	else return 0;
	// CalcQMSWR, first part
	b.QMSWR = CalcSWR(b.QMRi1, b.QMXi1, b.QMZo, 0);
	b.QMSWRL = CalcSWR(b.QMRL1, b.QMXL1, b.QMZo, 0);
	printf("\"tool\":\"line\"");
	num("ri", b.QMRi1); num("xi", b.QMXi1);
	num("rl", b.QMRL1); num("xl", b.QMXL1);
	num("swri", b.QMSWR); num("swrl", b.QMSWRL);
	return 1;
}

static void QMOut(TOptDlgBox &b)
{
	double Ri, Xi, Ri2, Xi2;
	b.CalcZi(Ri, Xi, b.QMZo2, b.QML2, b.QMRL2, b.QMXL2);
	b.CalcZi(Ri2, Xi2, b.QMZo1, b.QML1, Ri, Xi);
	b.CalcQMSWR();
	num("l1", b.QML1); num("l2", b.QML2);
	num("rj", Ri); num("xj", Xi);
	num("ri", Ri2); num("xi", Xi2);
	num("swr", b.QMSWR2);
}

static int QM(TOptDlgBox &b, int argc, char **argv, int tune)
{
	if( argc < (tune ? 5 : 7) ) return 0;
	b.QMRL2 = atof(argv[0]);
	b.QMXL2 = atof(argv[1]);
	b.QMRi2 = atof(argv[2]);
	b.QMRi1 = 50; b.QMXi1 = 0; b.QMRL1 = 50; b.QMXL1 = 0; b.QMZo = 50;
	if( tune ){
		b.QMZo1 = atof(argv[3]);
		b.QMZo2 = atof(argv[4]);
		b.QML1 = b.QML2 = 0;
		b.CalcQML();
		printf("\"tool\":\"qmtune\"");
		num("tuneswr", b.QMSWR2);
	}
	else {
		b.QMZo1 = atof(argv[3]);
		b.QML1 = atof(argv[4]);
		b.QMZo2 = atof(argv[5]);
		b.QML2 = atof(argv[6]);
		printf("\"tool\":\"qm\"");
	}
	QMOut(b);
	return 1;
}

static void SMSol(int k, double f, double cm, double L, double R, double X, double LS, double LO, double XC, double S, double Ri, double Xi)
{
	printf("%s{", k ? "," : "");
	num("l", L, 0); num("lcm", (29980.0/f)*cm*L);
	num("r", R); num("x", X);
	if( ABS(XC) >= 1e10 ) printf(",\"xc\":null");
	else {
		num("xc", XC);
		if( XC >= 0 ) num("uh", 0.1591549*XC/f); else num("pf", -159154.9/(f*XC));
		num("ls", LS); num("lscm", (29980.0/f)*cm*LS);
		num("lo", LO); num("locm", (29980.0/f)*cm*LO);
	}
	num("ri", Ri); num("xi", Xi); num("swr", S);
	printf("}");
}

static int SM(TOptDlgBox &b, int argc, char **argv)
{
	if( argc < 6 ) return 0;
	b.SMFreq = atof(argv[0]);
	b.SMZi = atof(argv[1]);
	b.SMR = atof(argv[2]);
	b.SMX = atof(argv[3]);
	b.SMZo = atof(argv[4]);
	b.SMCm = atof(argv[5]);
	b.CalcSM();
	printf("\"tool\":\"sm\",\"found\":%d,\"solutions\":[", b.SMF);
	if( b.SMF >= 1 ) SMSol(0, b.SMFreq, b.SMCm, b.SML1, b.SMR1, b.SMX1, b.SMLS1, b.SMLO1, b.SMXC1, b.SMSWR1, b.SMRi1, b.SMXi1);
	if( b.SMF >= 2 ) SMSol(1, b.SMFreq, b.SMCm, b.SML2, b.SMR2, b.SMX2, b.SMLS2, b.SMLO2, b.SMXC2, b.SMSWR2, b.SMRi2, b.SMXi2);
	printf("]");
	return 1;
}

// TOptDlgBox::AutojXClick without the input box: R is the SWR reference,
// MRO the impedance the hairpin matches to.
static int Hairpin(int argc, char **argv)
{
	if( argc < 2 ) return 0;
	double R = atof(argv[0]), MRO = atof(argv[1]);
	printf("\"tool\":\"hairpin\"");
	if( MRO > R ) num("jx", -(sqrt((MRO/R)-1) * R));
	else if( MRO == R ) num("jx", 0);
	else printf(",\"jx\":null");
	return 1;
}

static int BwMatch(int argc, char **argv)
{
	if( argc < 8 ) return 0;
	TBwDispDlg d;
	env.RO = atof(argv[0]);
	env.JXO = atof(argv[1]);
	d.MatchData.Flag = 1;
	d.MatchData.FQ = atof(argv[2]);
	d.MatchData.R = atof(argv[3]);
	d.MatchData.JX = atof(argv[4]);
	double f = atof(argv[5]), R = atof(argv[6]), X = atof(argv[7]);
	printf("\"tool\":\"bwmatch\"");
	exeenv.BwMatch = 1;
	num("swr", d.CalcSWR(f, R, X));
	exeenv.BwMatch = 2;
	double r = R, x = X;
	d.CalcZ(f, r, x);
	num("r", r); num("x", x);
	return 1;
}

int main(int argc, char *argv[])
{
	if( argc < 2 ){
		fprintf(stderr, "usage: tools <cases.txt>\n");
		return 2;
	}
	FILE *fp = fopen(argv[1], "rt");
	if( fp == NULL ){
		fprintf(stderr, "cannot open %s\n", argv[1]);
		return 2;
	}
	char line[1024];
	int first = 1;
	printf("[\n");
	while( fgets(line, sizeof(line), fp) != NULL ){
		char *h = strchr(line, '#');
		if( h ) *h = 0;
		char *av[16];
		int ac = 0;
		for( char *t = strtok(line, " \t\r\n"); t && (ac < 16); t = strtok(NULL, " \t\r\n") ) av[ac++] = t;
		if( !ac ) continue;
		TOptDlgBox b;
		printf("%s{", first ? "" : ",\n");
		first = 0;
		int ok;
		if( !strcmp(av[0], "tank") ) ok = Tank(b, ac-1, av+1);
		else if( !strcmp(av[0], "coil") ) ok = Coil(b, ac-1, av+1);
		else if( !strcmp(av[0], "match") ) ok = Match(b, ac-1, av+1);
		else if( !strcmp(av[0], "stub") ) ok = Stub(b, ac-1, av+1);
		else if( !strcmp(av[0], "line") ) ok = Line(b, ac-1, av+1);
		else if( !strcmp(av[0], "qm") ) ok = QM(b, ac-1, av+1, 0);
		else if( !strcmp(av[0], "qmtune") ) ok = QM(b, ac-1, av+1, 1);
		else if( !strcmp(av[0], "sm") ) ok = SM(b, ac-1, av+1);
		else if( !strcmp(av[0], "hairpin") ) ok = Hairpin(ac-1, av+1);
		else if( !strcmp(av[0], "bwmatch") ) ok = BwMatch(ac-1, av+1);
		else ok = 0;
		if( !ok ){
			fprintf(stderr, "bad case: %s\n", av[0]);
			return 2;
		}
		printf("}");
	}
	printf("\n]\n");
	fclose(fp);
	return 0;
}
