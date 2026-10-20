// Stand-in for the VCL main form in the optimisation oracle. CACal::Exec
// reaches the antenna, one trial (ACalExecute) and the log through MainWnd;
// the engine only logs through MainWnd->CalMemo->Lines->Add().
#ifndef MainH
#define MainH
#include "ComLib.h"

struct TStringsShim { void Add(const char *) {} void Clear(void) {} };
struct TMemoShim {
	TStringsShim LinesObj;
	TStringsShim *Lines;
	TMemoShim() : Lines(&LinesObj) {}
	void Clear(void) {}
};

// The antenna the oracle loaded (optimize_main.cpp).
ANTDEF *OracleAnt(void);

struct TMainWndShim {
	TMemoShim MemoObj;
	TMemoShim *CalMemo;
	ANTDEF &ant;
	TMainWndShim() : CalMemo(&MemoObj), ant(*OracleAnt()) {}
	int ACalExecute(void);
	void DrawPtnACalInfo(void);
};
extern TMainWndShim *MainWnd;

void LogPrint(LPCSTR fmt, ...);
LPCSTR StrDbl1(double d);
#define IDYES 6
#define IDNO 7

#endif
