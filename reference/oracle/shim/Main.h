// Stand-in for the VCL main form: the engine only logs through
// MainWnd->CalMemo->Lines->Add(), which the oracle discards.
#ifndef MainH
#define MainH
#include "ComLib.h"

struct TStringsShim { void Add(const char *) {} void Clear(void) {} };
struct TMemoShim { TStringsShim LinesObj; TStringsShim *Lines; TMemoShim() : Lines(&LinesObj) {} };
struct TMainWndShim { TMemoShim MemoObj; TMemoShim *CalMemo; TMainWndShim() : CalMemo(&MemoObj) {} };
extern TMainWndShim *MainWnd;

#endif
