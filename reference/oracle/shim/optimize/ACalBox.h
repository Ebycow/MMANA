// Stand-in for the optimisation setup dialog (ACalBox.h) and the element
// variable dialog (ACalEle.h): the members the extracted handlers use, with
// the grid reduced to its current row and the dialogs' answers preset.
#ifndef ACalBoxH
#define ACalBoxH
#include "Main.h"

#define __fastcall
struct TObject {};
struct TGridShim { int Row; };

class TACalDlg;

// What to register and link when an element has two or more of width,
// height and X width; the oracle sets the answer (`EleAnswer`).
class TACalEleBox {
public:
	TACalEleBox(TACalDlg *) {}
	int Execute(int &HW, int &VW, int &XW, int &HC, int &VC, int &XC);
};

class TACalDlg {
public:
	TGridShim GridObj;
	TGridShim *Grid;
	CACal *pACal;
	int WHFlag;
	int WHHW, WHVW, WHXW;
	int WHHC, WHVC, WHXC;
	TACalDlg(CACal *p) : Grid(&GridObj), pACal(p) { GridObj.Row = 0; }
	void UpdateView(void) {}
	void __fastcall K28Click(TObject *Sender);
	void __fastcall DelBtnClick(TObject *Sender);
	void __fastcall SetEleWith(int n, int w, double a, double b);
	void __fastcall AddEle(int n);
	void __fastcall AdjWithEle(void);
	void __fastcall AllEleClick(TObject *Sender);
	int __fastcall CheckSameWireChen(double ox, double oy, double oz);
	void __fastcall AddSameWireChen(int n);
	void __fastcall AddSameLoadChen(int n);
	void __fastcall K11Click(TObject *Sender);
	void __fastcall K34Click(TObject *Sender);
};

#endif
