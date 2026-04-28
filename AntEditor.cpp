//Copyright+LGPL
#include <vcl.h>
#pragma hdrstop

#include "AntEditor.h"
#include "AntView.h"

#include <memory.h>

//---------------------------------------------------------------------------
int AntEditorIsWireSelected(const ANTDEF *ap, const int *Selected, int SelectionCount,
	int CurrentRow, int Wire)
{
	if( (Wire < 0) || (Wire >= ap->wmax) ) return FALSE;
	if( SelectionCount > 0 ) return Selected[Wire];
	if( SelectionCount < 0 ) return FALSE;
	return (Wire == CurrentRow);
}

//---------------------------------------------------------------------------
int AntEditorSelectionCount(const ANTDEF *ap, int SelectionCount, int CurrentRow)
{
	if( SelectionCount > 0 ) return SelectionCount;
	if( SelectionCount < 0 ) return 0;
	return ((CurrentRow >= 0) && (CurrentRow < ap->wmax)) ? 1 : 0;
}

//---------------------------------------------------------------------------
void AntEditorClearSelection(int *Selected, int &SelectionCount)
{
	memset(Selected, 0, sizeof(int) * WMAX);
	SelectionCount = 0;
}

//---------------------------------------------------------------------------
void AntEditorDeselectSelection(int *Selected, int &SelectionCount)
{
	AntEditorClearSelection(Selected, SelectionCount);
	SelectionCount = -1;
}

//---------------------------------------------------------------------------
int AntEditorSelectionCenter(const ANTDEF *ap, const int *Selected, int SelectionCount,
	int CurrentRow, double &X, double &Y, double &Z)
{
	int cnt = 0;
	X = Y = Z = 0.0;
	for( int i = 0; i < ap->wmax; i++ ){
		if( !AntEditorIsWireSelected(ap, Selected, SelectionCount, CurrentRow, i) ) continue;
		const WDEF *wp = &ap->wdef[i];
		X += (wp->X1 + wp->X2) / 2.0;
		Y += (wp->Y1 + wp->Y2) / 2.0;
		Z += (wp->Z1 + wp->Z2) / 2.0;
		cnt++;
	}
	if( cnt <= 0 ) return FALSE;
	X /= double(cnt);
	Y /= double(cnt);
	Z /= double(cnt);
	return TRUE;
}

//---------------------------------------------------------------------------
int AntEditorDeleteSelectedWires(ANTDEF *ap, int *Selected, int &SelectionCount,
	int CurrentRow, int &FirstDeleted)
{
	if( AntEditorSelectionCount(ap, SelectionCount, CurrentRow) <= 0 ) return FALSE;

	int oldMax = ap->wmax;
	FirstDeleted = -1;
	int dst = 0;
	for( int i = 0; i < oldMax; i++ ){
		if( AntEditorIsWireSelected(ap, Selected, SelectionCount, CurrentRow, i) ){
			if( FirstDeleted < 0 ) FirstDeleted = i;
			continue;
		}
		if( dst != i ) memcpy(&ap->wdef[dst], &ap->wdef[i], sizeof(WDEF));
		dst++;
	}
	for( int i = dst; i < oldMax; i++ ){
		memset(&ap->wdef[i], 0, sizeof(WDEF));
	}
	ap->wmax = dst;
	ap->Edit = ap->Flag = 1;
	AntEditorClearSelection(Selected, SelectionCount);
	return TRUE;
}

//---------------------------------------------------------------------------
int AntEditorCopySelectedWires(const ANTDEF *ap, const int *Selected, int SelectionCount,
	int CurrentRow, WDEF *Clipboard, int &ClipboardCount)
{
	ClipboardCount = 0;
	for( int i = 0; i < ap->wmax; i++ ){
		if( !AntEditorIsWireSelected(ap, Selected, SelectionCount, CurrentRow, i) ) continue;
		memcpy(&Clipboard[ClipboardCount], &ap->wdef[i], sizeof(WDEF));
		ClipboardCount++;
	}
	return (ClipboardCount > 0) ? TRUE : FALSE;
}

//---------------------------------------------------------------------------
int AntEditorPasteWires(ANTDEF *ap, int *Selected, int &SelectionCount,
	const WDEF *Clipboard, int ClipboardCount, int &FirstPasted)
{
	if( ClipboardCount <= 0 ) return FALSE;
	if( ap->wmax + ClipboardCount > WMAX ) return FALSE;

	FirstPasted = ap->wmax;
	for( int i = 0; i < ClipboardCount; i++ ){
		memcpy(&ap->wdef[ap->wmax], &Clipboard[i], sizeof(WDEF));
		ap->wmax++;
	}
	AntEditorClearSelection(Selected, SelectionCount);
	for( int i = FirstPasted; i < ap->wmax; i++ ){
		Selected[i] = TRUE;
		SelectionCount++;
	}
	return TRUE;
}

//---------------------------------------------------------------------------
int AntEditorMirrorSelectedWires(ANTDEF *ap, const int *Selected, int SelectionCount,
	int CurrentRow, int Axis)
{
	double cx, cy, cz;
	if( AntEditorSelectionCenter(ap, Selected, SelectionCount, CurrentRow, cx, cy, cz) != TRUE ) return FALSE;

	for( int i = 0; i < ap->wmax; i++ ){
		if( !AntEditorIsWireSelected(ap, Selected, SelectionCount, CurrentRow, i) ) continue;
		WDEF *wp = &ap->wdef[i];
		switch( Axis ){
			case ANT_GIZMO_AXIS_X:
				wp->X1 = (2.0 * cx) - wp->X1;
				wp->X2 = (2.0 * cx) - wp->X2;
				break;
			case ANT_GIZMO_AXIS_Y:
				wp->Y1 = (2.0 * cy) - wp->Y1;
				wp->Y2 = (2.0 * cy) - wp->Y2;
				break;
			case ANT_GIZMO_AXIS_Z:
				wp->Z1 = (2.0 * cz) - wp->Z1;
				wp->Z2 = (2.0 * cz) - wp->Z2;
				break;
			default:
				return FALSE;
		}
	}
	ap->Edit = ap->Flag = 1;
	return TRUE;
}

//---------------------------------------------------------------------------
int AntEditorAlignSelectedWiresToOrigin(ANTDEF *ap, const int *Selected, int SelectionCount,
	int CurrentRow, int Axis)
{
	double cx, cy, cz;
	if( AntEditorSelectionCenter(ap, Selected, SelectionCount, CurrentRow, cx, cy, cz) != TRUE ) return FALSE;

	double dx = (Axis == ANT_GIZMO_AXIS_X) ? -cx : 0.0;
	double dy = (Axis == ANT_GIZMO_AXIS_Y) ? -cy : 0.0;
	if( (dx == 0.0) && (dy == 0.0) ) return TRUE;

	for( int i = 0; i < ap->wmax; i++ ){
		if( !AntEditorIsWireSelected(ap, Selected, SelectionCount, CurrentRow, i) ) continue;
		WDEF *wp = &ap->wdef[i];
		wp->X1 += dx;
		wp->X2 += dx;
		wp->Y1 += dy;
		wp->Y2 += dy;
	}
	ap->Edit = ap->Flag = 1;
	return TRUE;
}

//---------------------------------------------------------------------------
int AntEditorRotateSelectedWires90(ANTDEF *ap, const int *Selected, int SelectionCount,
	int CurrentRow)
{
	double cx, cy, cz;
	if( AntEditorSelectionCenter(ap, Selected, SelectionCount, CurrentRow, cx, cy, cz) != TRUE ) return FALSE;

	for( int i = 0; i < ap->wmax; i++ ){
		if( !AntEditorIsWireSelected(ap, Selected, SelectionCount, CurrentRow, i) ) continue;
		WDEF *wp = &ap->wdef[i];
		double x1 = wp->X1 - cx;
		double y1 = wp->Y1 - cy;
		double x2 = wp->X2 - cx;
		double y2 = wp->Y2 - cy;
		wp->X1 = cx - y1;
		wp->Y1 = cy + x1;
		wp->X2 = cx - y2;
		wp->Y2 = cy + x2;
	}
	ap->Edit = ap->Flag = 1;
	return TRUE;
}
