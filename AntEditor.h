//Copyright+LGPL
#ifndef AntEditorH
#define AntEditorH

#include "ComLib.h"

int AntEditorIsWireSelected(const ANTDEF *ap, const int *Selected, int SelectionCount,
	int CurrentRow, int Wire);
int AntEditorSelectionCount(const ANTDEF *ap, int SelectionCount, int CurrentRow);
void AntEditorClearSelection(int *Selected, int &SelectionCount);
void AntEditorDeselectSelection(int *Selected, int &SelectionCount);
int AntEditorSelectionCenter(const ANTDEF *ap, const int *Selected, int SelectionCount,
	int CurrentRow, double &X, double &Y, double &Z);
int AntEditorDeleteSelectedWires(ANTDEF *ap, int *Selected, int &SelectionCount,
	int CurrentRow, int &FirstDeleted);
int AntEditorCopySelectedWires(const ANTDEF *ap, const int *Selected, int SelectionCount,
	int CurrentRow, WDEF *Clipboard, int &ClipboardCount);
int AntEditorPasteWires(ANTDEF *ap, int *Selected, int &SelectionCount,
	const WDEF *Clipboard, int ClipboardCount, int &FirstPasted);
int AntEditorMirrorSelectedWires(ANTDEF *ap, const int *Selected, int SelectionCount,
	int CurrentRow, int Axis);
int AntEditorAlignSelectedWiresToOrigin(ANTDEF *ap, const int *Selected, int SelectionCount,
	int CurrentRow, int Axis);
int AntEditorRotateSelectedWires90(ANTDEF *ap, const int *Selected, int SelectionCount,
	int CurrentRow);

#endif
