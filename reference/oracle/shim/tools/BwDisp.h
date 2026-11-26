// Stand-in for the frequency-characteristics dialog (BwDisp.h): the members
// its matching calculations (AdjX, CalcSWR, CalcZ) use. MatchData has the
// fields of the original's BWS that they read; the dialog fills it with the
// antenna's impedance at the matching frequency (UpdateMatch).
#ifndef BwDispH
#define BwDispH
#include "ComLib.h"

#define __fastcall

struct BwMatchShim {
	int		Flag;
	double	FQ;			// MHz
	double	R;
	double	JX;
};

class TBwDispDlg {
public:
	BwMatchShim	MatchData;

	void __fastcall AdjX(double f, double fo, double &X);
	double __fastcall CalcSWR(double f, double R, double jX);
	void __fastcall CalcZ(double f, double &R, double &jX);
};
#endif
