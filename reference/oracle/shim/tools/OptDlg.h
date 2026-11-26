// Stand-in for the calculator dialog (OptDlg.h): the members the extracted
// calculations use, with the two radio groups they read reduced to their
// selected index. The fields are those of the original TOptDlgBox.
#ifndef OptDlgH
#define OptDlgH
#include "ComLib.h"

#define __fastcall

struct TRadioShim { int ItemIndex; };

class TOptDlgBox {
public:
	TRadioShim MXSelObj, CStubObj;
	TRadioShim *MXSel;		// LC match: 0 = shunt C, 1 = series C
	TRadioShim *CStub;		// stub: 0 = open, 1 = short
	TOptDlgBox() : MXSel(&MXSelObj), CStub(&CStubObj) { MXSelObj.ItemIndex = 0; CStubObj.ItemIndex = 0; }

	double	Freq;
	double	X;
	double	L;
	double	C;

	double	CoilL;
	double	CoilW;		// wire diameter (cm)
	double	CoilN;		// turns
	double	CoilS;		// spacing (cm)
	double	CoilD;		// coil diameter (cm)

	double	MF;
	double	MZ;
	double	MR;
	double	MX;

	int		MT;
	char	MM1[2];
	char	MM2[2];
	double	MX1;
	double	MX2;
	double	MD1;
	double	MD2;

	double	CoreFreq;
	int		CoreType;
	double	CoreP;
	double	CoreZ;
	double	CoreX;
	double	CoreL;
	double	CoreLC;
	double	CoreFo;

	double	QMRi1;
	double	QMXi1;
	double	QMRL1;
	double	QMXL1;
	double	QMZo;
	double	QML;
	double	QMA;

	double	QMRi2;
	double	QMRL2;
	double	QMXL2;
	double	QMZo1;
	double	QML1;
	double	QMZo2;
	double	QML2;
	double	QMSWR;
	double	QMSWRL;
	double	QMSWR2;

	int		SMF;
	double	SMR;
	double	SMX;
	double	SMZo;
	double	SMCm;
	double	SMFreq;
	double	SMZi;

	double	SML1;
	double	SMR1;
	double	SMX1;
	double	SMLS1;
	double	SMLO1;
	double	SMXC1;
	double	SMSWR1;
	double	SMRi1;
	double	SMXi1;
	double	SML2;
	double	SMR2;
	double	SMX2;
	double	SMLS2;
	double	SMLO2;
	double	SMXC2;
	double	SMSWR2;
	double	SMRi2;
	double	SMXi2;

	void __fastcall CalcTank(int sw);
	void __fastcall CalcCoil(int sw);
	void __fastcall CalcMatch(void);
	void __fastcall CalcCore(int n);

	void __fastcall CalcZi(double &Ri, double &Xi, double Zo, double L, double RL, double XL);
	void __fastcall CalcZL(double &RL, double &XL, double Zo, double L, double Ri, double Xi);
	void __fastcall CalcZi(double &Ri, double &Xi, double Zo, double L, double RL, double XL, double a);
	void __fastcall CalcZL(double &RL, double &XL, double Zo, double L, double Ri, double Xi, double a);

	void __fastcall CalcQML(void);
	void __fastcall CalcQMSWR(void);
	void __fastcall CopyQM(void);

	double __fastcall CalcSMStub(double L, int sw);
	void __fastcall CalcSM(void);
};

extern LPCSTR CoreTypeT[];
extern const float CorePT[];
extern const float CoreZT[];

#endif
