//Copyright+LGPL
#include <vcl.h>
#pragma hdrstop

#include "Main.h"
#include "mininec3.h"
#include "AntView.h"
#include "AntEditor.h"

//---------------------------------------------------------------------------
static void FormatAntLengthText(char *bf, LPCSTR Label, double Len)
{
	sprintf(bf, "%s:%s %s", Label, StrDbl(GetRmdVal(Len)), GetLenUnitText());
}
//---------------------------------------------------------------------------
static void PaintAntLengthLabel(TCanvas *Canvas, int Width, int Height, int X, int Y, LPCSTR Text)
{
	int tw = Canvas->TextWidth(Text);
	int th = Canvas->TextHeight(Text);
	int tx = X + 12;
	int ty = Y + 12;
	if( tx + tw + 8 > Width ) tx = X - tw - 12;
	if( ty + th + 6 > Height ) ty = Y - th - 12;
	if( tx < 0 ) tx = 0;
	if( ty < 0 ) ty = 0;

	TColor oldPen = Canvas->Pen->Color;
	TColor oldBrush = Canvas->Brush->Color;
	TColor oldFont = Canvas->Font->Color;
	TPenStyle oldPenStyle = Canvas->Pen->Style;
	TBrushStyle oldBrushStyle = Canvas->Brush->Style;
	TRect rc;
	rc.Left = tx - 3;
	rc.Top = ty - 2;
	rc.Right = tx + tw + 5;
	rc.Bottom = ty + th + 4;
	Canvas->Pen->Color = clBlack;
	Canvas->Pen->Style = psSolid;
	Canvas->Brush->Color = clInfoBk;
	Canvas->Brush->Style = bsSolid;
	Canvas->Rectangle(rc.Left, rc.Top, rc.Right, rc.Bottom);
	Canvas->Font->Color = clBlack;
	Canvas->TextOut(tx, ty, Text);
	Canvas->Pen->Color = oldPen;
	Canvas->Brush->Color = oldBrush;
	Canvas->Font->Color = oldFont;
	Canvas->Pen->Style = oldPenStyle;
	Canvas->Brush->Style = oldBrushStyle;
}

//---------------------------------------------------------------------------
// アンテナ形状の表示イベント
void __fastcall TMainWnd::PBoxAntPaint(TObject *Sender)
{
	TRect	rc;

	rc.Left = 0;
	rc.Top = 0;
	rc.Right = PBoxAnt->Width;
	rc.Bottom = PBoxAnt->Height;
	PBoxAnt->Canvas->Brush->Color = clWhite;
	PBoxAnt->Canvas->FillRect(rc);
	Clip.SetClip(-100, -100, rc.Right + 100, rc.Bottom + 100);
	double sc = double(TBarSC->Position) / 20.0;
	sc = sc * sc * sc * sc;
	int		Xc = int(PBoxAnt->Width/2 + (exeenv.AntXc * sc));
	int		Yc = int(PBoxAnt->Height/2 + (exeenv.AntYc * sc));

	PBoxAnt->Canvas->Pen->Color = clWhite;
	double x, y;
	int		X,Y, X2, Y2;
	double deg = double(TBarDeg->Position);
	deg *= (PAI / 180.0);
	double zdeg = double(TBarZDeg->Position);
	zdeg *= (PAI / 180.0);

	PBoxAnt->Canvas->Pen->Color = clLtGray;
	Calc3DXY(x, y, deg, zdeg, 0, 0, 0);				// Y
	X = int(x) + Xc;
	Y = Yc - int(y);
	PBoxAnt->Canvas->MoveTo(X, Y);
	Calc3DXY(x, y, deg, zdeg, 0, PBoxAnt->Width/2-16, 0);
	X = int(x) + Xc;
	Y = Yc - int(y);
	PBoxAnt->Canvas->LineTo(X, Y);
	int Sop = ::SetBkMode(PBoxAnt->Canvas->Handle, TRANSPARENT);
	PBoxAnt->Canvas->TextOut(X, Y, "Y");

	Calc3DXY(x, y, deg, zdeg, 0, 0, 0);				// Z
	X = int(x) + Xc;
	Y = Yc - int(y);
	PBoxAnt->Canvas->MoveTo(X, Y);
	Calc3DXY(x, y, deg, zdeg, 0, 0, PBoxAnt->Height);
	X = int(x) + Xc;
	Y = Yc - int(y);
	PBoxAnt->Canvas->LineTo(X, Y);
	::SetBkMode(PBoxAnt->Canvas->Handle, TRANSPARENT);
	PBoxAnt->Canvas->TextOut(X+4, 0, "Z");

	Calc3DXY(x, y, deg, zdeg, 0, 0, 0);				// X
	X = int(x) + Xc;
	Y = Yc - int(y);
	PBoxAnt->Canvas->MoveTo(X, Y);
	Calc3DXY(x, y, deg, zdeg, PBoxAnt->Height/2, 0, 0);
	X = int(x) + Xc;
	Y = Yc - int(y);
	PBoxAnt->Canvas->LineTo(X, Y);
	::SetBkMode(PBoxAnt->Canvas->Handle, TRANSPARENT);
	PBoxAnt->Canvas->TextOut(X, Y, "X");

	// ワイヤの表示
	PBoxAnt->Canvas->Pen->Color = clBlack;
	int i;
	WDEF *wp = pCalAnt->wdef;
	for( i = 0; i < pCalAnt->wmax; i++, wp++ ){
		PBoxAnt->Canvas->Pen->Color = wp->R ? clBlack : clGray;
		int sw = ant.wmax ? (i % ant.wmax) : i;
		PBoxAnt->Canvas->Pen->Width = IsAntWireSelected(sw) ? 2 : 1;
		CalcAntViewXY(x, y, deg, zdeg, wp->X1, wp->Y1, wp->Z1);
		X = int((x * sc)) + Xc;
		Y = Yc - int((y * sc));
		CalcAntViewXY(x, y, deg, zdeg, wp->X2, wp->Y2, wp->Z2);
		X2 = int((x * sc)) + Xc;
		Y2 = Yc - int((y * sc));
		if( Clip.Clip(X, Y, X2, Y2) == TRUE ){
			PBoxAnt->Canvas->MoveTo(X, Y);
			PBoxAnt->Canvas->LineTo(X2, Y2);
		}
		if( wp->R < 0.0 ){	// 組み合わせワイヤ
			PDEF *pp = FindPP(pCalAnt, wp->R);
			if( pp != NULL ){
				PBoxAnt->Canvas->Pen->Width = 1;
				PBoxAnt->Canvas->Pen->Color = clBlue;
				WDEF	temp[PPMAX*2+2];
				int n = CombWire(temp, pCalAnt, pp, wp) - 1;
				int j;
				WDEF *tp;
				for( tp = temp, j = 0; j < n; j++, tp++ ){
					CalcAntViewXY(x, y, deg, zdeg, tp->X2, tp->Y2, tp->Z2);
					X = int((x * sc)) + Xc;
					Y = Yc - int((y * sc));
					::SetBkMode(PBoxAnt->Canvas->Handle, TRANSPARENT);
					PBoxAnt->Canvas->MoveTo(X-2, Y-2);
					PBoxAnt->Canvas->LineTo(X+2, Y-2); PBoxAnt->Canvas->LineTo(X+2, Y+2);
					PBoxAnt->Canvas->LineTo(X-2, Y+2); PBoxAnt->Canvas->LineTo(X-2, Y-2);
				}
				PBoxAnt->Canvas->Pen->Color = clBlack;
			}
		}
	}
	PBoxAnt->Canvas->Pen->Width = 1;
	PaintAntEditGizmo();
	PaintAntSnapVertices();
	PaintAntDrawPreview();
	PaintAntDrawSnapPoint();
	double	cx, cy, cz;
	if( pCalAnt->wmax && DspPlus->Checked ){		// セグメント分割の表示
		PBoxAnt->Canvas->Pen->Color = clGreen;
		for( i = 1; i <= (GetPlusMax()+(pCalAnt->wzmax*2)); i++ ){
			GetSegPos(cx, cy, cz, i);
			CalcAntViewXY(x, y, deg, zdeg, cx, cy, cz);
			X = int((x * sc)) + Xc;
			Y = Yc - int((y * sc));
			::SetBkMode(PBoxAnt->Canvas->Handle, TRANSPARENT);
			PBoxAnt->Canvas->MoveTo(X-3, Y-3); PBoxAnt->Canvas->LineTo(X+3, Y+3);
			PBoxAnt->Canvas->MoveTo(X+3, Y-3); PBoxAnt->Canvas->LineTo(X-3, Y+3);
		}
	}
	int	YT = 0;
	PBoxAnt->Canvas->Pen->Color = clRed;
	int FH = PBoxAnt->Canvas->TextHeight("給電点");
	PBoxAnt->Canvas->TextOut(12, YT, "給電点");
	PBoxAnt->Canvas->Ellipse(3, YT + (FH/4), 10, YT + (FH/4) + 7);
	YT += FH + 2;
	for( i = 0; i < pCalAnt->cmax; i++ ){	// 給電点
		int PlusNo = Str2PlusNo(pCalAnt, pCalAnt->cdef[i].PLUS);
		if( PlusNo <= 0 ) continue;
		GetSegPos(cx, cy, cz, Plus2Seg(PlusNo));
		CalcAntViewXY(x, y, deg, zdeg, cx, cy, cz);
		X = int((x * sc)) + Xc;
		Y = Yc - int((y * sc));
		::SetBkMode(PBoxAnt->Canvas->Handle, TRANSPARENT);
		PBoxAnt->Canvas->Ellipse(X-3, Y-3, X+4, Y+4);
	}

	PBoxAnt->Canvas->Pen->Color = clRed;
	Y = YT+(FH/2);
	PBoxAnt->Canvas->MoveTo(3, Y-3); PBoxAnt->Canvas->LineTo(9, Y+3);
	PBoxAnt->Canvas->MoveTo(9, Y-3); PBoxAnt->Canvas->LineTo(3, Y+3);
	PBoxAnt->Canvas->TextOut(12, YT, "集中定数");
	if( EnbLoad->Checked ){
		for( i = 0; i < pCalAnt->lmax; i++ ){	// ロード
			int PlusNo = Str2PlusNo(pCalAnt, pCalAnt->ldef[i].PLUS);
			if( PlusNo <= 0 ) continue;
			GetSegPos(cx, cy, cz, Plus2Seg(PlusNo));
			CalcAntViewXY(x, y, deg, zdeg, cx, cy, cz);
			X = int((x * sc)) + Xc;
			Y = Yc - int((y * sc));
			::SetBkMode(PBoxAnt->Canvas->Handle, TRANSPARENT);
			PBoxAnt->Canvas->MoveTo(X-3, Y-3); PBoxAnt->Canvas->LineTo(X+3, Y+3);
			PBoxAnt->Canvas->MoveTo(X+3, Y-3); PBoxAnt->Canvas->LineTo(X-3, Y+3);
		}
	}
#if 0
	YT += FH + 2;
	PBoxAnt->Canvas->Pen->Color = clBlue;
	X = 5; Y = YT+(FH/2);
	PBoxAnt->Canvas->MoveTo(X-2, Y-2);
	PBoxAnt->Canvas->LineTo(X+2, Y-2); PBoxAnt->Canvas->LineTo(X+2, Y+2);
	PBoxAnt->Canvas->LineTo(X-2, Y+2); PBoxAnt->Canvas->LineTo(X-2, Y-2);
	PBoxAnt->Canvas->TextOut(12, YT, "組み合わせ点");
#endif

	// 電流分布の表示
	if( DspCur->Checked && res.IsCalc() ){
		int s, p, v;
		int AX=0;
		int AY=0;
		int	wno = -1;
		double Mag = double(TBarCur->Position) / 10.0;
		Mag = Mag * Mag * Mag * Mag;
		double	Vect, Cur;
		for( i = 0; i < pCalAnt->wzmax; i++ ){
			if( !pCalAnt->wzdef[i].PNo ) continue;
			s = pCalAnt->wzdef[i].Wno;
			Vect = ABS(pCalAnt->wdef[s].Z2 - pCalAnt->wdef[s].Z1);
			if( (Vect >= ABS(pCalAnt->wdef[s].Y2 - pCalAnt->wdef[s].Y1))&&
				(Vect >= ABS(pCalAnt->wdef[s].X2 - pCalAnt->wdef[s].X1)) ){
				v = 1;
			}
			else {
				v = 0;
			}
			if( exeenv.CurDir ){
				Vect = (pCalAnt->wdef[s].X2 - pCalAnt->wdef[s].X1) + (pCalAnt->wdef[s].Y2 - pCalAnt->wdef[s].Y1) + (pCalAnt->wdef[s].Z2 - pCalAnt->wdef[s].Z1);
			}
			for( s = pCalAnt->wzdef[i].SNo, p = pCalAnt->wzdef[i].PNo; s <= pCalAnt->wzdef[i].SMax; s++, p++ ){
				GetSegPos(cx, cy, cz, s);
				if( res.Cur[p-1] == NULLF ) continue;
				Cur = res.Cur[p-1] * Mag;
				if( exeenv.CurDir ){
					Cur = (Vect >= 0) ? Cur : -Cur;
				}
				else {
					Cur = ABS(Cur);
				}
				if( v ){
					cy += Cur;
					PBoxAnt->Canvas->Pen->Color = clRed;
				}
				else {
					cz += Cur;
					PBoxAnt->Canvas->Pen->Color = clBlue;
				}
				CalcAntViewXY(x, y, deg, zdeg, cx, cy, cz);
				X = int((x * sc)) + Xc;
				Y = Yc - int((y * sc));
				ClipXY(X, Y);
				if( wno != pCalAnt->wzdef[i].Wno ){
					wno = pCalAnt->wzdef[i].Wno;
				}
				else {
					::SetBkMode(PBoxAnt->Canvas->Handle, TRANSPARENT);
					PBoxAnt->Canvas->MoveTo(AX, AY); PBoxAnt->Canvas->LineTo(X, Y);
				}
				AX = X; AY = Y;
			}
		}
	}
	int SelWire = Grid2->Row - 1;
	if( (GetAntSelectionCount() > 0) && (SelWire >= 0) && (SelWire < ant.wmax) ){
		DrawWirePara(PBoxAnt, &ant, SelWire);
	}
	if( (ant.StackH > 1)||(ant.StackV > 1) ){
		char bf[32];
		sprintf(bf, "%u×%u スタック", ant.StackH, ant.StackV);
		X = PBoxAnt->Width - PBoxAnt->Canvas->TextWidth(bf);
		PBoxAnt->Canvas->Font->Color = clRed;
		PBoxAnt->Canvas->TextOut(X-2, 0+2, bf);
		PBoxAnt->Canvas->Font->Color = clBlack;
	}
	::SetBkMode(PBoxAnt->Canvas->Handle, Sop);
}
//---------------------------------------------------------------------------
// マウスによるワイヤの選択
int __fastcall TMainWnd::SelectWire(int X, int Y)
{
	int r = FALSE;
	// ワイヤの検索
	double sc = double(TBarSC->Position) / 20.0;
	sc = sc * sc * sc * sc;
	int	Xc = int(PBoxAnt->Width/2 + (exeenv.AntXc * sc));
	int	Yc = int(PBoxAnt->Height/2 + (exeenv.AntYc * sc));
	double x, y;
	double deg = double(TBarDeg->Position);
	deg *= (PAI / 180.0);
	double zdeg = double(TBarZDeg->Position);
	zdeg *= (PAI / 180.0);
	POINT	PP, BP, EP;
	PP.x = X; PP.y = Y;
	int i;
	int mw = 9;
	int w, mi;
	for( i = 0; i < pCalAnt->wmax; i++ ){
		CalcAntViewXY(x, y, deg, zdeg, pCalAnt->wdef[i].X1, pCalAnt->wdef[i].Y1, pCalAnt->wdef[i].Z1);
		BP.x = int((x * sc)) + Xc;
		BP.y = Yc - int((y * sc));
		CalcAntViewXY(x, y, deg, zdeg, pCalAnt->wdef[i].X2, pCalAnt->wdef[i].Y2, pCalAnt->wdef[i].Z2);
		EP.x = int((x * sc)) + Xc;
		EP.y = Yc - int((y * sc));
		for( w = 1; w <= mw; w++ ){
			if( IsLine(PP, BP, EP, w) != 0 ){
				mw = w;
				mi = i;
				break;
			}
		}
	}
	if( mw != 9 ){
		if( mi >= ant.wmax ) mi %= ant.wmax;
		if( PBoxAntClickCtrl ) ToggleAntWireSelection(mi);
		else SelectOnlyAntWire(mi);
		r = TRUE;
	}
	return r;
}
//---------------------------------------------------------------------------
// アンテナ形状表示のマウス操作設定
int __fastcall TMainWnd::GetAntMouseAction(TMouseButton Button)
{
	if( Button == mbLeft ) return exeenv.AntMouseLeft;
	if( Button == mbMiddle ) return exeenv.AntMouseMiddle;
	return ANT_MOUSE_NONE;
}
//---------------------------------------------------------------------------
double __fastcall TMainWnd::GetAntViewScale(void)
{
	return AntViewScaleFromTrack(TBarSC->Position);
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::SetTrackBarPosition(TTrackBar *Bar, int Pos)
{
	if( Pos < Bar->Min ) Pos = Bar->Min;
	if( Pos > Bar->Max ) Pos = Bar->Max;
	if( Bar->Position != Pos ) Bar->Position = Pos;
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::SetTrackBarPositionWrapped(TTrackBar *Bar, int Pos)
{
	int range = Bar->Max - Bar->Min + 1;
	if( range <= 0 ){
		SetTrackBarPosition(Bar, Pos);
		return;
	}
	while( Pos < Bar->Min ) Pos += range;
	while( Pos > Bar->Max ) Pos -= range;
	if( Bar->Position != Pos ) Bar->Position = Pos;
}
//---------------------------------------------------------------------------
double __fastcall TMainWnd::GetAntViewUnitScale(void)
{
	return AntViewUnitScale(exeenv.RmdSel, exeenv.MmSel);
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::CalcAntViewXY(double &x, double &y, double deg, double zdeg, double X, double Y, double Z)
{
	AntViewProject(x, y, deg, zdeg, GetAntViewUnitScale(), X, Y, Z);
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::AntViewPan(int DX, int DY)
{
	double sc = GetAntViewScale();
	if( sc <= 0.0 ) return;
	exeenv.AntXc += double(DX) / sc;
	exeenv.AntYc += double(DY) / sc;
	PBoxAnt->Invalidate();
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::AntViewRotate(int DX, int DY)
{
	SetTrackBarPositionWrapped(TBarDeg, TBarDeg->Position + DX);
	if( exeenv.Ant3D ){
		SetTrackBarPosition(TBarZDeg, TBarZDeg->Position - DY);
	}
	PBoxAnt->Invalidate();
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::AntViewRotateDrag(int X, int Y)
{
	int dx = X - PBoxAntMX;
	int dy = Y - PBoxAntMY;
	int deg = PBoxAntStartDeg + AntViewRoundMouseAngle(AntViewMouseDragToAngle(dx, PBoxAnt->Width));
	SetTrackBarPositionWrapped(TBarDeg, deg);
	if( exeenv.Ant3D ){
		int zdeg = PBoxAntStartZDeg - AntViewRoundMouseAngle(AntViewMouseDragToAngle(dy, PBoxAnt->Height));
		SetTrackBarPosition(TBarZDeg, zdeg);
	}
	PBoxAnt->Invalidate();
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::ClearAntRedo(void)
{
	AntRedoList->Clear();
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::PushAntUndo(void)
{
	AnsiString snap;
	SaveAntStrings(snap);
	if( AntUndoList->Count && (AntUndoList->Strings[AntUndoList->Count - 1] == snap) ) return;
	AntUndoList->Add(snap);
	while( AntUndoList->Count > 64 ) AntUndoList->Delete(0);
	ClearAntRedo();
}
//---------------------------------------------------------------------------
int __fastcall TMainWnd::RestoreAntSnapshot(TStringList *From, TStringList *To)
{
	if( From->Count <= 0 ) return FALSE;
	AnsiString cur;
	SaveAntStrings(cur);
	To->Add(cur);
	while( To->Count > 64 ) To->Delete(0);

	int n = From->Count - 1;
	AnsiString snap = From->Strings[n];
	From->Delete(n);
	LoadAntStrings(snap);
	Grid2->EditorMode = FALSE;
	Grid3->EditorMode = FALSE;
	Grid4->EditorMode = FALSE;
	SetAntDef();
	ant.Edit = ant.Flag = 1;
	exeenv.CalcLog = 1;
	SetStackAnt();
	res.ClearBWC();
	Grid2->Invalidate();
	Grid3->Invalidate();
	Grid4->Invalidate();
	UpdateAllViews();
	RequestAutoCalc(FALSE);
	return TRUE;
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::UndoAntEdit(void)
{
	if( RestoreAntSnapshot(AntUndoList, AntRedoList) != TRUE ) ::MessageBeep(MB_ICONEXCLAMATION);
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::RedoAntEdit(void)
{
	if( RestoreAntSnapshot(AntRedoList, AntUndoList) != TRUE ) ::MessageBeep(MB_ICONEXCLAMATION);
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::DeleteSelectedAntWires(void)
{
	if( GetAntSelectionCount() <= 0 ){
		::MessageBeep(MB_ICONEXCLAMATION);
		return;
	}
	PushAntUndo();
	int firstDeleted = -1;
	if( AntEditorDeleteSelectedWires(&ant, AntWireSelected, AntWireSelectionCount, Grid2->Row - 1, firstDeleted) != TRUE ){
		::MessageBeep(MB_ICONEXCLAMATION);
		return;
	}
	AntGizmoShowSnapVertices = FALSE;
	UpdateCount();
	Grid2->RowCount = ant.wmax + 2;
	if( ant.wmax ){
		if( firstDeleted >= ant.wmax ) firstDeleted = ant.wmax - 1;
		Grid2->Row = firstDeleted + 1;
	}
	else {
		Grid2->Row = 1;
	}
	Grid2->Invalidate();
	PBoxAnt->Invalidate();
	UpdateAntPreview();
	RequestAutoCalc(FALSE);
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::ClearAntWireSelection(void)
{
	AntEditorClearSelection(AntWireSelected, AntWireSelectionCount);
	AntGizmoShowSnapVertices = FALSE;
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::DeselectAntWireSelection(void)
{
	AntEditorDeselectSelection(AntWireSelected, AntWireSelectionCount);
	AntGizmoShowSnapVertices = FALSE;
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::SelectOnlyAntWire(int Wire)
{
	ClearAntWireSelection();
	if( (Wire >= 0) && (Wire < ant.wmax) ){
		AntWireSelected[Wire] = TRUE;
		AntWireSelectionCount = 1;
		Grid2->Row = Wire + 1;
		Grid2->SetFocus();
	}
	PBoxAnt->Invalidate();
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::ToggleAntWireSelection(int Wire)
{
	if( (Wire < 0) || (Wire >= ant.wmax) ) return;
	if( AntWireSelectionCount < 0 ){
		AntWireSelectionCount = 0;
	}
	else if( AntWireSelectionCount == 0 ){
		int cur = Grid2->Row - 1;
		if( (cur >= 0) && (cur < ant.wmax) && (cur != Wire) ){
			AntWireSelected[cur] = TRUE;
			AntWireSelectionCount = 1;
		}
	}
	if( AntWireSelected[Wire] ){
		AntWireSelected[Wire] = FALSE;
		if( AntWireSelectionCount > 0 ) AntWireSelectionCount--;
	}
	else {
		AntWireSelected[Wire] = TRUE;
		AntWireSelectionCount++;
		Grid2->Row = Wire + 1;
		Grid2->SetFocus();
	}
	PBoxAnt->Invalidate();
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::SelectAllAntWires(void)
{
	if( ant.wmax <= 0 ) return;
	for( int i = 0; i < ant.wmax; i++ ) AntWireSelected[i] = TRUE;
	AntWireSelectionCount = ant.wmax;
	Grid2->Row = 1;
	Grid2->SetFocus();
	PBoxAnt->Invalidate();
}
//---------------------------------------------------------------------------
int __fastcall TMainWnd::GetAntSelectionCount(void)
{
	return AntEditorSelectionCount(&ant, AntWireSelectionCount, Grid2->Row - 1);
}
//---------------------------------------------------------------------------
int __fastcall TMainWnd::IsAntWireSelected(int Wire)
{
	return AntEditorIsWireSelected(&ant, AntWireSelected, AntWireSelectionCount, Grid2->Row - 1, Wire);
}
//---------------------------------------------------------------------------
int __fastcall TMainWnd::GetAntSelectionCenter(double &X, double &Y, double &Z)
{
	return AntEditorSelectionCenter(&ant, AntWireSelected, AntWireSelectionCount, Grid2->Row - 1, X, Y, Z);
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::CopyAntWires(void)
{
	if( AntEditorCopySelectedWires(&ant, AntWireSelected, AntWireSelectionCount, Grid2->Row - 1, AntWireClipboard, AntWireClipboardCount) != TRUE ){
		::MessageBeep(MB_ICONEXCLAMATION);
	}
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::PasteAntWires(void)
{
	if( AntWireClipboardCount <= 0 ){
		::MessageBeep(MB_ICONEXCLAMATION);
		return;
	}
	if( ant.wmax + AntWireClipboardCount > WMAX ){
		::MessageBeep(MB_ICONEXCLAMATION);
		return;
	}
	PushAntUndo();
	int first = 0;
	if( AntEditorPasteWires(&ant, AntWireSelected, AntWireSelectionCount, AntWireClipboard, AntWireClipboardCount, first) != TRUE ){
		::MessageBeep(MB_ICONEXCLAMATION);
		return;
	}
	Grid2->RowCount = ant.wmax + 2;
	Grid2->Row = first + 1;
	UpdateAntData();
	Grid2->Row = first + 1;
	PBoxAnt->Invalidate();
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::MirrorSelectedWires(int Axis)
{
	if( GetAntSelectionCount() <= 0 ){
		::MessageBeep(MB_ICONEXCLAMATION);
		return;
	}
	PushAntUndo();
	if( AntEditorMirrorSelectedWires(&ant, AntWireSelected, AntWireSelectionCount, Grid2->Row - 1, Axis) != TRUE ){
		::MessageBeep(MB_ICONEXCLAMATION);
		return;
	}
	UpdateAntData();
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::MirrorSelectedXClick(TObject *Sender)
{
	MirrorSelectedWires(ANT_GIZMO_AXIS_X);
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::MirrorSelectedYClick(TObject *Sender)
{
	MirrorSelectedWires(ANT_GIZMO_AXIS_Y);
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::MirrorSelectedZClick(TObject *Sender)
{
	MirrorSelectedWires(ANT_GIZMO_AXIS_Z);
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::AlignSelectedWiresToOrigin(int Axis)
{
	if( GetAntSelectionCount() <= 0 ){
		::MessageBeep(MB_ICONEXCLAMATION);
		return;
	}
	PushAntUndo();
	if( AntEditorAlignSelectedWiresToOrigin(&ant, AntWireSelected, AntWireSelectionCount, Grid2->Row - 1, Axis) != TRUE ){
		::MessageBeep(MB_ICONEXCLAMATION);
		return;
	}
	UpdateAntData();
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::RotateSelectedWires90(void)
{
	if( GetAntSelectionCount() <= 0 ){
		::MessageBeep(MB_ICONEXCLAMATION);
		return;
	}
	PushAntUndo();
	if( AntEditorRotateSelectedWires90(&ant, AntWireSelected, AntWireSelectionCount, Grid2->Row - 1) != TRUE ){
		::MessageBeep(MB_ICONEXCLAMATION);
		return;
	}
	UpdateAntData();
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::AlignSelectedX0Click(TObject *Sender)
{
	AlignSelectedWiresToOrigin(ANT_GIZMO_AXIS_X);
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::AlignSelectedY0Click(TObject *Sender)
{
	AlignSelectedWiresToOrigin(ANT_GIZMO_AXIS_Y);
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::RotateSelected90Click(TObject *Sender)
{
	RotateSelectedWires90();
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::CreateAntDrawControls(void)
{
	if( AntDrawBtn != NULL ) return;

	AntDrawBtn = new TButton(this);
	AntDrawBtn->Parent = TabSheet2;
	AntDrawBtn->Caption = "Draw";
	AntDrawBtn->TabStop = false;
	AntDrawBtn->OnClick = AntDrawWireToggle;

	AntDrawXYBtn = new TButton(this);
	AntDrawXYBtn->Parent = TabSheet2;
	AntDrawXYBtn->Caption = "XY";
	AntDrawXYBtn->TabStop = false;
	AntDrawXYBtn->OnClick = AntDrawPlaneClick;

	AntDrawXZBtn = new TButton(this);
	AntDrawXZBtn->Parent = TabSheet2;
	AntDrawXZBtn->Caption = "XZ";
	AntDrawXZBtn->TabStop = false;
	AntDrawXZBtn->OnClick = AntDrawPlaneClick;

	AntDrawYZBtn = new TButton(this);
	AntDrawYZBtn->Parent = TabSheet2;
	AntDrawYZBtn->Caption = "YZ";
	AntDrawYZBtn->TabStop = false;
	AntDrawYZBtn->OnClick = AntDrawPlaneClick;

	AntDrawCancelBtn = new TButton(this);
	AntDrawCancelBtn->Parent = TabSheet2;
	AntDrawCancelBtn->Caption = "Cancel";
	AntDrawCancelBtn->TabStop = false;
	AntDrawCancelBtn->OnClick = AntDrawCancelClick;

	AntAlignX0Btn = new TButton(this);
	AntAlignX0Btn->Parent = TabSheet2;
	AntAlignX0Btn->Caption = "X0";
	AntAlignX0Btn->TabStop = false;
	AntAlignX0Btn->Hint = "Move selected wires to X=0";
	AntAlignX0Btn->ShowHint = true;
	AntAlignX0Btn->OnClick = AlignSelectedX0Click;

	AntAlignY0Btn = new TButton(this);
	AntAlignY0Btn->Parent = TabSheet2;
	AntAlignY0Btn->Caption = "Y0";
	AntAlignY0Btn->TabStop = false;
	AntAlignY0Btn->Hint = "Move selected wires to Y=0";
	AntAlignY0Btn->ShowHint = true;
	AntAlignY0Btn->OnClick = AlignSelectedY0Click;

	AntRotate90Btn = new TButton(this);
	AntRotate90Btn->Parent = TabSheet2;
	AntRotate90Btn->Caption = "R90";
	AntRotate90Btn->TabStop = false;
	AntRotate90Btn->Hint = "Rotate selected wires 90 degrees";
	AntRotate90Btn->ShowHint = true;
	AntRotate90Btn->OnClick = RotateSelected90Click;

	LayoutAntDrawControls();
	UpdateAntDrawControls();
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::LayoutAntDrawControls(void)
{
	if( (AntDrawBtn == NULL) || (AntAlignX0Btn == NULL) || (AllViewBtn == NULL) || (OrgBtn == NULL) ) return;

	const int gap = 4;
	const int drawW = 54;
	const int planeW = 32;
	const int cancelW = 50;
	const int alignW = 36;
	const int rotateW = 44;
	int top = OrgBtn->Top;
	int h = OrgBtn->Height;

	if( QuadMode && (PanelTopRight != NULL) ){
		top = 2;
		h = 25;
		int right = PanelTopRight->ClientWidth - gap;
		OrgBtn->SetBounds(right - 91, top, 91, h);
		right -= 91 + gap;
		AllViewBtn->SetBounds(right - 91, top, 91, h);

		OrgBtn->Font->Assign(Font);
		AllViewBtn->Font->Assign(Font);
		AntDrawBtn->Font->Assign(Font);
		AntDrawXYBtn->Font->Assign(Font);
		AntDrawXZBtn->Font->Assign(Font);
		AntDrawYZBtn->Font->Assign(Font);
		AntDrawCancelBtn->Font->Assign(Font);
		AntAlignX0Btn->Font->Assign(Font);
		AntAlignY0Btn->Font->Assign(Font);
		AntRotate90Btn->Font->Assign(Font);
	}

	int total = drawW + gap + planeW + gap + planeW + gap + planeW + gap +
		cancelW + gap + alignW + gap + alignW + gap + rotateW;
	int left = AllViewBtn->Left - gap - total;
	if( left < 2 ) left = 2;

	AntDrawBtn->SetBounds(left, top, drawW, h);
	left += drawW + gap;
	AntDrawXYBtn->SetBounds(left, top, planeW, h);
	left += planeW + gap;
	AntDrawXZBtn->SetBounds(left, top, planeW, h);
	left += planeW + gap;
	AntDrawYZBtn->SetBounds(left, top, planeW, h);
	left += planeW + gap;
	AntDrawCancelBtn->SetBounds(left, top, cancelW, h);
	left += cancelW + gap;
	AntAlignX0Btn->SetBounds(left, top, alignW, h);
	left += alignW + gap;
	AntAlignY0Btn->SetBounds(left, top, alignW, h);
	left += alignW + gap;
	AntRotate90Btn->SetBounds(left, top, rotateW, h);
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::UpdateAntDrawControls(void)
{
	if( AntDrawBtn == NULL ) return;

	bool enabled = exeenv.Ant3D ? true : false;
	bool selected = (GetAntSelectionCount() > 0) ? true : false;
	AntDrawBtn->Enabled = enabled;
	AntDrawXYBtn->Enabled = enabled;
	AntDrawXZBtn->Enabled = enabled;
	AntDrawYZBtn->Enabled = enabled;
	AntDrawCancelBtn->Enabled = enabled && AntDrawMode;
	AntAlignX0Btn->Enabled = selected;
	AntAlignY0Btn->Enabled = selected;
	AntRotate90Btn->Enabled = selected;

	AntDrawBtn->Caption = AntDrawMode ? "[Draw]" : "Draw";
	AntDrawXYBtn->Caption = (AntDrawPlane == ANT_DRAW_PLANE_XY) ? "[XY]" : "XY";
	AntDrawXZBtn->Caption = (AntDrawPlane == ANT_DRAW_PLANE_XZ) ? "[XZ]" : "XZ";
	AntDrawYZBtn->Caption = (AntDrawPlane == ANT_DRAW_PLANE_YZ) ? "[YZ]" : "YZ";

	if( KAntDrawWire != NULL ){
		KAntDrawWire->Checked = AntDrawMode ? true : false;
		KAntDrawWire->Enabled = enabled;
	}
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::SetAntDrawPlane(int Plane)
{
	if( (Plane < ANT_DRAW_PLANE_XY) || (Plane > ANT_DRAW_PLANE_YZ) ) return;
	if( AntDrawPlane != Plane ){
		AntDrawPlane = Plane;
		AntDrawActive = FALSE;
		AntDrawSnapVisible = FALSE;
	}
	UpdateAntDrawControls();
	PBoxAnt->Invalidate();
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::AntDrawPlaneClick(TObject *Sender)
{
	if( Sender == AntDrawXYBtn ) SetAntDrawPlane(ANT_DRAW_PLANE_XY);
	else if( Sender == AntDrawXZBtn ) SetAntDrawPlane(ANT_DRAW_PLANE_XZ);
	else if( Sender == AntDrawYZBtn ) SetAntDrawPlane(ANT_DRAW_PLANE_YZ);
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::AntDrawCancelClick(TObject *Sender)
{
	if( AntDrawActive ){
		AntDrawActive = FALSE;
		AntDrawSnapVisible = FALSE;
		PBoxAnt->Invalidate();
	}
	else {
		SetAntDrawMode(FALSE);
	}
	UpdateAntDrawControls();
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::SetAntDrawMode(int Enabled)
{
	if( Enabled && !exeenv.Ant3D ){
		::MessageBeep(MB_ICONEXCLAMATION);
		Enabled = FALSE;
	}
	AntDrawMode = Enabled ? TRUE : FALSE;
	AntDrawActive = FALSE;
	AntDrawSnapVisible = FALSE;
	PBoxAntDragButton = -1;
	PBoxAntDragAction = ANT_MOUSE_NONE;
	PBoxAnt->Cursor = AntDrawMode ? crCross : crDefault;
	UpdateAntDrawControls();
	PBoxAnt->Invalidate();
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::AntDrawWireToggle(TObject *Sender)
{
	SetAntDrawMode(!AntDrawMode);
}
//---------------------------------------------------------------------------
int __fastcall TMainWnd::AntViewPointToXY(int X, int Y, double &WX, double &WY, double &WZ)
{
	if( !exeenv.Ant3D ) return FALSE;
	TAntViewParams view;
	view.Width = PBoxAnt->Width;
	view.Height = PBoxAnt->Height;
	view.CenterX = exeenv.AntXc;
	view.CenterY = exeenv.AntYc;
	view.Deg = TBarDeg->Position;
	view.ZDeg = TBarZDeg->Position;
	view.Scale = GetAntViewScale();
	view.UnitScale = GetAntViewUnitScale();
	TAntDrawPlaneState plane;
	plane.Plane = AntDrawPlane;
	plane.Active = AntDrawActive;
	plane.X1 = AntDrawX1;
	plane.Y1 = AntDrawY1;
	plane.Z1 = AntDrawZ1;
	return AntViewScreenToPlane(view, plane, X, Y, WX, WY, WZ);
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::AntWorldToScreen(double WX, double WY, double WZ, int &X, int &Y)
{
	TAntViewParams view;
	view.Width = PBoxAnt->Width;
	view.Height = PBoxAnt->Height;
	view.CenterX = exeenv.AntXc;
	view.CenterY = exeenv.AntYc;
	view.Deg = TBarDeg->Position;
	view.ZDeg = TBarZDeg->Position;
	view.Scale = GetAntViewScale();
	view.UnitScale = GetAntViewUnitScale();
	AntViewWorldToScreen(view, WX, WY, WZ, X, Y);
}
//---------------------------------------------------------------------------
int __fastcall TMainWnd::SnapAntDrawPoint(int X, int Y, double &WX, double &WY, double &WZ)
{
	TAntViewParams view;
	view.Width = PBoxAnt->Width;
	view.Height = PBoxAnt->Height;
	view.CenterX = exeenv.AntXc;
	view.CenterY = exeenv.AntYc;
	view.Deg = TBarDeg->Position;
	view.ZDeg = TBarZDeg->Position;
	view.Scale = GetAntViewScale();
	view.UnitScale = GetAntViewUnitScale();
	AntDrawSnapVisible = FALSE;
	int found = AntViewFindSnapPoint(&ant, view, X, Y, WX, WY, WZ);
	if( found ){
		AntDrawSnapVisible = TRUE;
		AntDrawSnapX = WX;
		AntDrawSnapY = WY;
		AntDrawSnapZ = WZ;
	}
	return found;
}
//---------------------------------------------------------------------------
int __fastcall TMainWnd::GetAntGizmoAxisScreen(double WX, double WY, double WZ, int Axis, int Len,
	int &X1, int &Y1, int &X2, int &Y2, double &DX, double &DY)
{
	TAntViewParams view;
	view.Width = PBoxAnt->Width;
	view.Height = PBoxAnt->Height;
	view.CenterX = exeenv.AntXc;
	view.CenterY = exeenv.AntYc;
	view.Deg = TBarDeg->Position;
	view.ZDeg = TBarZDeg->Position;
	view.Scale = GetAntViewScale();
	view.UnitScale = GetAntViewUnitScale();
	return AntViewGizmoAxisScreen(view, WX, WY, WZ, Axis, Len, X1, Y1, X2, Y2, DX, DY);
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::PaintAntEditGizmo(void)
{
	if( AntDrawMode || !exeenv.Ant3D ) return;
	int SelCount = GetAntSelectionCount();
	if( SelCount <= 0 ) return;

	TColor oldColor = PBoxAnt->Canvas->Pen->Color;
	TPenStyle oldStyle = PBoxAnt->Canvas->Pen->Style;
	int oldWidth = PBoxAnt->Canvas->Pen->Width;
	TColor oldBrush = PBoxAnt->Canvas->Brush->Color;
	TBrushStyle oldBrushStyle = PBoxAnt->Canvas->Brush->Style;

	if( SelCount > 1 ){
		double wx, wy, wz;
		if( GetAntSelectionCenter(wx, wy, wz) != TRUE ) return;
		int sx, sy;
		AntWorldToScreen(wx, wy, wz, sx, sy);
		PBoxAnt->Canvas->Pen->Color = clBlack;
		PBoxAnt->Canvas->Pen->Style = psSolid;
		PBoxAnt->Canvas->Pen->Width = 1;
		PBoxAnt->Canvas->Brush->Style = bsClear;
		PBoxAnt->Canvas->Rectangle(sx-5, sy-5, sx+6, sy+6);
		for( int axis = 0; axis < 3; axis++ ){
			int x1, y1, x2, y2;
			double dx, dy;
			if( GetAntGizmoAxisScreen(wx, wy, wz, axis, 46, x1, y1, x2, y2, dx, dy) != TRUE ) continue;
			switch( axis ){
				case ANT_GIZMO_AXIS_X: PBoxAnt->Canvas->Pen->Color = clRed; break;
				case ANT_GIZMO_AXIS_Y: PBoxAnt->Canvas->Pen->Color = clGreen; break;
				case ANT_GIZMO_AXIS_Z: PBoxAnt->Canvas->Pen->Color = clBlue; break;
			}
			PBoxAnt->Canvas->Pen->Width = (AntGizmoDrag && (AntGizmoEndpoint == 3) && (AntGizmoAxis == axis)) ? 3 : 2;
			AntViewDrawScreenArrow(PBoxAnt->Canvas, x1, y1, x2, y2);
		}
		PaintAntGizmoMoveDistance();

		PBoxAnt->Canvas->Pen->Color = oldColor;
		PBoxAnt->Canvas->Pen->Style = oldStyle;
		PBoxAnt->Canvas->Pen->Width = oldWidth;
		PBoxAnt->Canvas->Brush->Color = oldBrush;
		PBoxAnt->Canvas->Brush->Style = oldBrushStyle;
		return;
	}

	int w = Grid2->Row - 1;
	if( (w < 0) || (w >= ant.wmax) ) return;
	WDEF *wp = &ant.wdef[w];
	for( int endp = 0; endp < 3; endp++ ){
		double wx = endp ? wp->X2 : wp->X1;
		double wy = endp ? wp->Y2 : wp->Y1;
		double wz = endp ? wp->Z2 : wp->Z1;
		if( endp == 2 ){
			wx = (wp->X1 + wp->X2) / 2.0;
			wy = (wp->Y1 + wp->Y2) / 2.0;
			wz = (wp->Z1 + wp->Z2) / 2.0;
		}
		int sx, sy;
		AntWorldToScreen(wx, wy, wz, sx, sy);
		PBoxAnt->Canvas->Pen->Color = clBlack;
		PBoxAnt->Canvas->Pen->Style = psSolid;
		PBoxAnt->Canvas->Pen->Width = 1;
		PBoxAnt->Canvas->Brush->Style = bsClear;
		PBoxAnt->Canvas->Ellipse(sx-4, sy-4, sx+5, sy+5);

		for( int axis = 0; axis < 3; axis++ ){
			int x1, y1, x2, y2;
			double dx, dy;
			if( GetAntGizmoAxisScreen(wx, wy, wz, axis, 42, x1, y1, x2, y2, dx, dy) != TRUE ) continue;
			switch( axis ){
				case ANT_GIZMO_AXIS_X: PBoxAnt->Canvas->Pen->Color = clRed; break;
				case ANT_GIZMO_AXIS_Y: PBoxAnt->Canvas->Pen->Color = clGreen; break;
				case ANT_GIZMO_AXIS_Z: PBoxAnt->Canvas->Pen->Color = clBlue; break;
			}
			PBoxAnt->Canvas->Pen->Width = (AntGizmoDrag && (AntGizmoWire == w) &&
				(AntGizmoEndpoint == endp) && (AntGizmoAxis == axis)) ? 3 : 2;
			AntViewDrawScreenArrow(PBoxAnt->Canvas, x1, y1, x2, y2);
		}
	}

	PaintAntGizmoMoveDistance();

	PBoxAnt->Canvas->Pen->Color = oldColor;
	PBoxAnt->Canvas->Pen->Style = oldStyle;
	PBoxAnt->Canvas->Pen->Width = oldWidth;
	PBoxAnt->Canvas->Brush->Color = oldBrush;
	PBoxAnt->Canvas->Brush->Style = oldBrushStyle;
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::PaintAntSnapVertices(void)
{
	if( !AntGizmoShowSnapVertices ) return;
	TColor oldColor = PBoxAnt->Canvas->Pen->Color;
	TColor oldBrush = PBoxAnt->Canvas->Brush->Color;
	TBrushStyle oldBrushStyle = PBoxAnt->Canvas->Brush->Style;
	PBoxAnt->Canvas->Pen->Color = clFuchsia;
	PBoxAnt->Canvas->Brush->Color = clFuchsia;
	PBoxAnt->Canvas->Brush->Style = bsSolid;
	for( int i = 0; i < ant.wmax; i++ ){
		if( IsAntWireSelected(i) ) continue;
		int x, y;
		AntWorldToScreen(ant.wdef[i].X1, ant.wdef[i].Y1, ant.wdef[i].Z1, x, y);
		PBoxAnt->Canvas->Ellipse(x-3, y-3, x+4, y+4);
		AntWorldToScreen(ant.wdef[i].X2, ant.wdef[i].Y2, ant.wdef[i].Z2, x, y);
		PBoxAnt->Canvas->Ellipse(x-3, y-3, x+4, y+4);
	}
	PBoxAnt->Canvas->Pen->Color = oldColor;
	PBoxAnt->Canvas->Brush->Color = oldBrush;
	PBoxAnt->Canvas->Brush->Style = oldBrushStyle;
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::PaintAntDrawSnapPoint(void)
{
	if( !AntDrawMode || !AntDrawSnapVisible ) return;
	int x, y;
	AntWorldToScreen(AntDrawSnapX, AntDrawSnapY, AntDrawSnapZ, x, y);
	TColor oldPen = PBoxAnt->Canvas->Pen->Color;
	TColor oldBrush = PBoxAnt->Canvas->Brush->Color;
	TPenStyle oldStyle = PBoxAnt->Canvas->Pen->Style;
	int oldWidth = PBoxAnt->Canvas->Pen->Width;
	TBrushStyle oldBrushStyle = PBoxAnt->Canvas->Brush->Style;

	PBoxAnt->Canvas->Pen->Color = clBlue;
	PBoxAnt->Canvas->Pen->Style = psSolid;
	PBoxAnt->Canvas->Pen->Width = 2;
	PBoxAnt->Canvas->Brush->Color = clAqua;
	PBoxAnt->Canvas->Brush->Style = bsSolid;
	PBoxAnt->Canvas->Ellipse(x-5, y-5, x+6, y+6);
	PBoxAnt->Canvas->MoveTo(x-8, y);
	PBoxAnt->Canvas->LineTo(x+9, y);
	PBoxAnt->Canvas->MoveTo(x, y-8);
	PBoxAnt->Canvas->LineTo(x, y+9);

	PBoxAnt->Canvas->Pen->Color = oldPen;
	PBoxAnt->Canvas->Brush->Color = oldBrush;
	PBoxAnt->Canvas->Pen->Style = oldStyle;
	PBoxAnt->Canvas->Pen->Width = oldWidth;
	PBoxAnt->Canvas->Brush->Style = oldBrushStyle;
}
//---------------------------------------------------------------------------
int __fastcall TMainWnd::FindAntSnapVertex(int X, int Y, double &WX, double &WY, double &WZ)
{
	TAntViewParams view;
	view.Width = PBoxAnt->Width;
	view.Height = PBoxAnt->Height;
	view.CenterX = exeenv.AntXc;
	view.CenterY = exeenv.AntYc;
	view.Deg = TBarDeg->Position;
	view.ZDeg = TBarZDeg->Position;
	view.Scale = GetAntViewScale();
	view.UnitScale = GetAntViewUnitScale();
	return AntViewFindSnapVertex(&ant, view, AntWireSelected, AntWireSelectionCount, Grid2->Row - 1, X, Y, WX, WY, WZ);
}
//---------------------------------------------------------------------------
int __fastcall TMainWnd::FindAntSnapEdge(int X, int Y, double &WX, double &WY, double &WZ)
{
	TAntViewParams view;
	view.Width = PBoxAnt->Width;
	view.Height = PBoxAnt->Height;
	view.CenterX = exeenv.AntXc;
	view.CenterY = exeenv.AntYc;
	view.Deg = TBarDeg->Position;
	view.ZDeg = TBarZDeg->Position;
	view.Scale = GetAntViewScale();
	view.UnitScale = GetAntViewUnitScale();
	return AntViewFindSnapEdge(&ant, view, AntWireSelected, AntWireSelectionCount, Grid2->Row - 1, X, Y, WX, WY, WZ);
}
//---------------------------------------------------------------------------
int __fastcall TMainWnd::HitAntEditGizmo(int X, int Y, int &Endpoint, int &Axis)
{
	if( AntDrawMode || !exeenv.Ant3D ) return FALSE;
	int SelCount = GetAntSelectionCount();
	if( SelCount <= 0 ) return FALSE;

	double best = (8.0 * 8.0) + 1.0;
	int found = FALSE;
	if( SelCount > 1 ){
		double wx, wy, wz;
		if( GetAntSelectionCenter(wx, wy, wz) == TRUE ){
			int sx, sy;
			AntWorldToScreen(wx, wy, wz, sx, sy);
			int mdx = sx - X;
			int mdy = sy - Y;
			if( ((mdx * mdx) + (mdy * mdy)) <= (7 * 7) ){
				Endpoint = 3;
				Axis = -1;
				return TRUE;
			}
			for( int axis = 0; axis < 3; axis++ ){
				int x1, y1, x2, y2;
				double dx, dy;
				if( GetAntGizmoAxisScreen(wx, wy, wz, axis, 46, x1, y1, x2, y2, dx, dy) != TRUE ) continue;
				double d = AntViewSqDistPointToSegment(X, Y, x1, y1, x2, y2);
				if( d < best ){
					best = d;
					Endpoint = 3;
					Axis = axis;
					found = TRUE;
				}
			}
		}
		return found;
	}

	int w = Grid2->Row - 1;
	if( (w < 0) || (w >= ant.wmax) ) return FALSE;
	WDEF *wp = &ant.wdef[w];
	for( int endp = 0; endp < 3; endp++ ){
		double wx = endp ? wp->X2 : wp->X1;
		double wy = endp ? wp->Y2 : wp->Y1;
		double wz = endp ? wp->Z2 : wp->Z1;
		if( endp == 2 ){
			wx = (wp->X1 + wp->X2) / 2.0;
			wy = (wp->Y1 + wp->Y2) / 2.0;
			wz = (wp->Z1 + wp->Z2) / 2.0;
		}
		int sx, sy;
		AntWorldToScreen(wx, wy, wz, sx, sy);
		int mdx = sx - X;
		int mdy = sy - Y;
		if( ((mdx * mdx) + (mdy * mdy)) <= (7 * 7) ){
			Endpoint = endp;
			Axis = -1;
			return TRUE;
		}
		for( int axis = 0; axis < 3; axis++ ){
			int x1, y1, x2, y2;
			double dx, dy;
			if( GetAntGizmoAxisScreen(wx, wy, wz, axis, 42, x1, y1, x2, y2, dx, dy) != TRUE ) continue;
			double d = AntViewSqDistPointToSegment(X, Y, x1, y1, x2, y2);
			if( d < best ){
				best = d;
				Endpoint = endp;
				Axis = axis;
				found = TRUE;
			}
		}
	}
	return found;
}
//---------------------------------------------------------------------------
int __fastcall TMainWnd::BeginAntGizmoDrag(int X, int Y)
{
	int endpoint, axis;
	if( HitAntEditGizmo(X, Y, endpoint, axis) != TRUE ) return FALSE;
	PushAntUndo();
	int w = Grid2->Row - 1;
	if( (w < 0) || (w >= ant.wmax) ) return FALSE;

	WDEF *wp = &ant.wdef[w];
	double wx = endpoint ? wp->X2 : wp->X1;
	double wy = endpoint ? wp->Y2 : wp->Y1;
	double wz = endpoint ? wp->Z2 : wp->Z1;
	if( endpoint == 3 ){
		GetAntSelectionCenter(wx, wy, wz);
	}
	else if( endpoint == 2 ){
		wx = (wp->X1 + wp->X2) / 2.0;
		wy = (wp->Y1 + wp->Y2) / 2.0;
		wz = (wp->Z1 + wp->Z2) / 2.0;
	}
	int x1, y1, x2, y2;
	double dx = 0.0;
	double dy = 0.0;
	if( axis >= 0 ){
		if( GetAntGizmoAxisScreen(wx, wy, wz, axis, 42, x1, y1, x2, y2, dx, dy) != TRUE ) return FALSE;
	}

	AntGizmoDrag = TRUE;
	AntGizmoEndpoint = endpoint;
	AntGizmoAxis = axis;
	AntGizmoWire = w;
	AntGizmoMouseX = X;
	AntGizmoMouseY = Y;
	AntGizmoAxisDX = dx;
	AntGizmoAxisDY = dy;
	memcpy(&AntGizmoOldW, wp, sizeof(WDEF));
	memcpy(AntGizmoOldSelected, ant.wdef, sizeof(AntGizmoOldSelected));
	PBoxAntIgnoreClick = TRUE;
	PBoxAnt->Cursor = crSizeAll;
	PBoxAnt->Invalidate();
	return TRUE;
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::GetAntGizmoAnchor(double &X, double &Y, double &Z)
{
	X = AntGizmoOldW.X1;
	Y = AntGizmoOldW.Y1;
	Z = AntGizmoOldW.Z1;
	if( AntGizmoEndpoint == 1 ){
		X = AntGizmoOldW.X2;
		Y = AntGizmoOldW.Y2;
		Z = AntGizmoOldW.Z2;
	}
	else if( AntGizmoEndpoint == 2 ){
		X = (AntGizmoOldW.X1 + AntGizmoOldW.X2) / 2.0;
		Y = (AntGizmoOldW.Y1 + AntGizmoOldW.Y2) / 2.0;
		Z = (AntGizmoOldW.Z1 + AntGizmoOldW.Z2) / 2.0;
	}
	else if( AntGizmoEndpoint == 3 ){
		X = Y = Z = 0.0;
		int cnt = 0;
		for( int i = 0; i < ant.wmax; i++ ){
			if( !IsAntWireSelected(i) ) continue;
			X += (AntGizmoOldSelected[i].X1 + AntGizmoOldSelected[i].X2) / 2.0;
			Y += (AntGizmoOldSelected[i].Y1 + AntGizmoOldSelected[i].Y2) / 2.0;
			Z += (AntGizmoOldSelected[i].Z1 + AntGizmoOldSelected[i].Z2) / 2.0;
			cnt++;
		}
		if( cnt > 0 ){
			X /= double(cnt);
			Y /= double(cnt);
			Z /= double(cnt);
		}
	}
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::GetAntGizmoFreeDelta(int X, int Y, double &DX, double &DY, double &DZ)
{
	DX = DY = DZ = 0.0;
	double sx, sy, sz;
	GetAntGizmoAnchor(sx, sy, sz);

	int x0, y0, xx, yx, xy, yy, xz, yz;
	AntWorldToScreen(sx, sy, sz, x0, y0);
	AntWorldToScreen(sx + 1.0, sy, sz, xx, yx);
	AntWorldToScreen(sx, sy + 1.0, sz, xy, yy);
	AntWorldToScreen(sx, sy, sz + 1.0, xz, yz);
	double bx0 = double(xx - x0);
	double by0 = double(yx - y0);
	double bx1 = double(xy - x0);
	double by1 = double(yy - y0);
	double bx2 = double(xz - x0);
	double by2 = double(yz - y0);
	double mx = double(X - AntGizmoMouseX);
	double my = double(Y - AntGizmoMouseY);
	double a = (bx0 * bx0) + (bx1 * bx1) + (bx2 * bx2);
	double b = (bx0 * by0) + (bx1 * by1) + (bx2 * by2);
	double c = (by0 * by0) + (by1 * by1) + (by2 * by2);
	double det = (a * c) - (b * b);
	if( ABS(det) < 1.0e-9 ) return;
	double px = ((c * mx) - (b * my)) / det;
	double py = ((a * my) - (b * mx)) / det;
	DX = (bx0 * px) + (by0 * py);
	DY = (bx1 * px) + (by1 * py);
	DZ = (bx2 * px) + (by2 * py);
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::UpdateAntGizmoDrag(int X, int Y, TShiftState Shift)
{
	if( !AntGizmoDrag ) return;
	if( (AntGizmoWire < 0) || (AntGizmoWire >= ant.wmax) ) return;
	WDEF *wp = &ant.wdef[AntGizmoWire];
	double mx = double(X - AntGizmoMouseX);
	double my = double(Y - AntGizmoMouseY);
	bool ctrl = Shift.Contains(ssCtrl);
	bool shift = Shift.Contains(ssShift);
	AntGizmoShowSnapVertices = shift && !ctrl;
	double sx, sy, sz;
	GetAntGizmoAnchor(sx, sy, sz);
	if( AntGizmoAxis < 0 ){
		int dragX = X;
		int dragY = Y;
		if( ctrl && shift ){
			if( ABS(mx) >= ABS(my) ) dragY = AntGizmoMouseY;
			else dragX = AntGizmoMouseX;
		}
		double dx, dy, dz;
		GetAntGizmoFreeDelta(dragX, dragY, dx, dy, dz);
		int rx, ry;
		AntWorldToScreen(sx + dx, sy + dy, sz + dz, rx, ry);
		double tx, ty, tz;
		if( shift && !ctrl && (FindAntSnapVertex(rx, ry, tx, ty, tz) == TRUE) ){
			dx = tx - sx;
			dy = ty - sy;
			dz = tz - sz;
		}
		else if( ctrl && !shift && (FindAntSnapEdge(rx, ry, tx, ty, tz) == TRUE) ){
			dx = tx - sx;
			dy = ty - sy;
			dz = tz - sz;
		}
		if( AntGizmoEndpoint == 0 ){
			wp->X1 = AntGizmoOldW.X1 + dx;
			wp->Y1 = AntGizmoOldW.Y1 + dy;
			wp->Z1 = AntGizmoOldW.Z1 + dz;
		}
		else if( AntGizmoEndpoint == 1 ){
			wp->X2 = AntGizmoOldW.X2 + dx;
			wp->Y2 = AntGizmoOldW.Y2 + dy;
			wp->Z2 = AntGizmoOldW.Z2 + dz;
		}
		else if( AntGizmoEndpoint == 2 ){
			memcpy(wp, &AntGizmoOldW, sizeof(WDEF));
			wp->X1 += dx; wp->Y1 += dy; wp->Z1 += dz;
			wp->X2 += dx; wp->Y2 += dy; wp->Z2 += dz;
		}
		else if( AntGizmoEndpoint == 3 ){
			for( int i = 0; i < ant.wmax; i++ ){
				if( !IsAntWireSelected(i) ) continue;
				memcpy(&ant.wdef[i], &AntGizmoOldSelected[i], sizeof(WDEF));
				ant.wdef[i].X1 += dx; ant.wdef[i].Y1 += dy; ant.wdef[i].Z1 += dz;
				ant.wdef[i].X2 += dx; ant.wdef[i].Y2 += dy; ant.wdef[i].Z2 += dz;
			}
		}
		ant.Edit = ant.Flag = 1;
		Grid2->Invalidate();
		PBoxAnt->Invalidate();
		return;
	}
	double len2 = (AntGizmoAxisDX * AntGizmoAxisDX) + (AntGizmoAxisDY * AntGizmoAxisDY);
	if( len2 <= 0.0 ) return;

	if( ctrl && shift ){
		if( ABS(mx) >= ABS(my) ) my = 0.0;
		else mx = 0.0;
	}
	double d = ((mx * AntGizmoAxisDX) + (my * AntGizmoAxisDY)) / len2;
	double ax = (AntGizmoAxis == ANT_GIZMO_AXIS_X) ? 1.0 : 0.0;
	double ay = (AntGizmoAxis == ANT_GIZMO_AXIS_Y) ? 1.0 : 0.0;
	double az = (AntGizmoAxis == ANT_GIZMO_AXIS_Z) ? 1.0 : 0.0;
	int rx, ry;
	AntWorldToScreen(sx + (ax * d), sy + (ay * d), sz + (az * d), rx, ry);
	double tx, ty, tz;
	if( shift && !ctrl && (FindAntSnapVertex(rx, ry, tx, ty, tz) == TRUE) ){
		switch( AntGizmoAxis ){
			case ANT_GIZMO_AXIS_X: d = tx - sx; break;
			case ANT_GIZMO_AXIS_Y: d = ty - sy; break;
			case ANT_GIZMO_AXIS_Z: d = tz - sz; break;
		}
	}
	else if( ctrl && !shift && (FindAntSnapEdge(rx, ry, tx, ty, tz) == TRUE) ){
		switch( AntGizmoAxis ){
			case ANT_GIZMO_AXIS_X: d = tx - sx; break;
			case ANT_GIZMO_AXIS_Y: d = ty - sy; break;
			case ANT_GIZMO_AXIS_Z: d = tz - sz; break;
		}
	}
	if( AntGizmoEndpoint == 0 ){
		wp->X1 = AntGizmoOldW.X1;
		wp->Y1 = AntGizmoOldW.Y1;
		wp->Z1 = AntGizmoOldW.Z1;
		switch( AntGizmoAxis ){
			case ANT_GIZMO_AXIS_X: wp->X1 += d; break;
			case ANT_GIZMO_AXIS_Y: wp->Y1 += d; break;
			case ANT_GIZMO_AXIS_Z: wp->Z1 += d; break;
		}
	}
	else if( AntGizmoEndpoint == 1 ){
		wp->X2 = AntGizmoOldW.X2;
		wp->Y2 = AntGizmoOldW.Y2;
		wp->Z2 = AntGizmoOldW.Z2;
		switch( AntGizmoAxis ){
			case ANT_GIZMO_AXIS_X: wp->X2 += d; break;
			case ANT_GIZMO_AXIS_Y: wp->Y2 += d; break;
			case ANT_GIZMO_AXIS_Z: wp->Z2 += d; break;
		}
	}
	else if( AntGizmoEndpoint == 2 ){
		memcpy(wp, &AntGizmoOldW, sizeof(WDEF));
		switch( AntGizmoAxis ){
			case ANT_GIZMO_AXIS_X: wp->X1 += d; wp->X2 += d; break;
			case ANT_GIZMO_AXIS_Y: wp->Y1 += d; wp->Y2 += d; break;
			case ANT_GIZMO_AXIS_Z: wp->Z1 += d; wp->Z2 += d; break;
		}
	}
	else {
		for( int i = 0; i < ant.wmax; i++ ){
			if( !IsAntWireSelected(i) ) continue;
			memcpy(&ant.wdef[i], &AntGizmoOldSelected[i], sizeof(WDEF));
			switch( AntGizmoAxis ){
				case ANT_GIZMO_AXIS_X: ant.wdef[i].X1 += d; ant.wdef[i].X2 += d; break;
				case ANT_GIZMO_AXIS_Y: ant.wdef[i].Y1 += d; ant.wdef[i].Y2 += d; break;
				case ANT_GIZMO_AXIS_Z: ant.wdef[i].Z1 += d; ant.wdef[i].Z2 += d; break;
			}
		}
	}
	ant.Edit = ant.Flag = 1;
	Grid2->Invalidate();
	PBoxAnt->Invalidate();
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::EndAntGizmoDrag(void)
{
	if( !AntGizmoDrag ) return;
	int w = AntGizmoWire;
	AntGizmoDrag = FALSE;
	AntGizmoWire = -1;
	AntGizmoShowSnapVertices = FALSE;
	PBoxAnt->Cursor = AntDrawMode ? crCross : crDefault;
	if( (w >= 0) && (w < ant.wmax) ){
		if( AntGizmoEndpoint == 3 ){
			UpdateAntData();
			RequestAutoCalc(TRUE);
			return;
		}
		if( ChkWith->Checked == TRUE ){
			AdjWireChen(ant.wdef, ant.wmax, &ant.wdef[w], &AntGizmoOldW);
		}
		UpdateAntData();
		RequestAutoCalc(TRUE);
	}
	else {
		PBoxAnt->Invalidate();
	}
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::PaintAntGizmoMoveDistance(void)
{
	if( !AntGizmoDrag ) return;
	if( (AntGizmoWire < 0) || (AntGizmoWire >= ant.wmax) ) return;

	double ox, oy, oz;
	double cx, cy, cz;
	GetAntGizmoAnchor(ox, oy, oz);
	WDEF *wp = &ant.wdef[AntGizmoWire];
	if( AntGizmoEndpoint == 0 ){
		cx = wp->X1;
		cy = wp->Y1;
		cz = wp->Z1;
	}
	else if( AntGizmoEndpoint == 1 ){
		cx = wp->X2;
		cy = wp->Y2;
		cz = wp->Z2;
	}
	else if( AntGizmoEndpoint == 2 ){
		cx = (wp->X1 + wp->X2) / 2.0;
		cy = (wp->Y1 + wp->Y2) / 2.0;
		cz = (wp->Z1 + wp->Z2) / 2.0;
	}
	else if( GetAntSelectionCenter(cx, cy, cz) != TRUE ){
		return;
	}

	double dx = cx - ox;
	double dy = cy - oy;
	double dz = cz - oz;
	double dist = sqrt((dx * dx) + (dy * dy) + (dz * dz));
	int sx, sy;
	AntWorldToScreen(cx, cy, cz, sx, sy);

	char bf[160];
	FormatAntLengthText(bf, "Move", dist);
	PaintAntLengthLabel(PBoxAnt->Canvas, PBoxAnt->Width, PBoxAnt->Height, sx, sy, bf);
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::PaintAntDrawPreview(void)
{
	if( !AntDrawMode || !AntDrawActive ) return;
	int X1, Y1, X2, Y2;
	AntWorldToScreen(AntDrawX1, AntDrawY1, AntDrawZ1, X1, Y1);
	AntWorldToScreen(AntDrawX2, AntDrawY2, AntDrawZ2, X2, Y2);

	TColor oldColor = PBoxAnt->Canvas->Pen->Color;
	TPenStyle oldStyle = PBoxAnt->Canvas->Pen->Style;
	int oldWidth = PBoxAnt->Canvas->Pen->Width;
	PBoxAnt->Canvas->Pen->Color = clRed;
	PBoxAnt->Canvas->Pen->Style = psDash;
	PBoxAnt->Canvas->Pen->Width = 1;
	PBoxAnt->Canvas->MoveTo(X1, Y1);
	PBoxAnt->Canvas->LineTo(X2, Y2);
	PBoxAnt->Canvas->Pen->Style = psSolid;
	PBoxAnt->Canvas->Ellipse(X1-3, Y1-3, X1+4, Y1+4);
	PBoxAnt->Canvas->Ellipse(X2-3, Y2-3, X2+4, Y2+4);
	PBoxAnt->Canvas->Pen->Color = oldColor;
	PBoxAnt->Canvas->Pen->Style = oldStyle;
	PBoxAnt->Canvas->Pen->Width = oldWidth;

	double dx = AntDrawX2 - AntDrawX1;
	double dy = AntDrawY2 - AntDrawY1;
	double dz = AntDrawZ2 - AntDrawZ1;
	double dist = sqrt((dx * dx) + (dy * dy) + (dz * dz));
	char bf[160];
	FormatAntLengthText(bf, "Length", dist);
	PaintAntLengthLabel(PBoxAnt->Canvas, PBoxAnt->Width, PBoxAnt->Height, (X1 + X2) / 2, (Y1 + Y2) / 2, bf);
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::AddAntDrawWire(double X1, double Y1, double Z1, double X2, double Y2, double Z2)
{
	PushAntUndo();
	if( ant.wmax >= WMAX ){
		::MessageBeep(MB_ICONEXCLAMATION);
		return;
	}
	if( (ABS(X2-X1) + ABS(Y2-Y1) + ABS(Z2-Z1)) < 1.0e-9 ) return;

	int n = ant.wmax;
	WDEF *wp = &ant.wdef[n];
	memset(wp, 0, sizeof(WDEF));
	wp->X1 = X1;
	wp->Y1 = Y1;
	wp->Z1 = Z1;
	wp->X2 = X2;
	wp->Y2 = Y2;
	wp->Z2 = Z2;
	wp->R = 0.0008;
	wp->SEG = 0;
	if( n ){
		if( ant.wdef[n-1].R > 0.0 ) wp->R = ant.wdef[n-1].R;
		wp->SEG = ant.wdef[n-1].SEG;
	}
	ant.wmax++;
	Grid2->RowCount = ant.wmax + 2;
	Grid2->Row = n + 1;
	UpdateAntData();
	if( n < (Grid2->RowCount - 1) ) Grid2->Row = n + 1;
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::AntViewZoom(int Delta, int X, int Y)
{
	double oldSc = GetAntViewScale();
	int oldPos = TBarSC->Position;
	SetTrackBarPosition(TBarSC, TBarSC->Position + Delta);
	if( oldPos == TBarSC->Position ) return;
	double newSc = GetAntViewScale();
	if( (oldSc > 0.0) && (newSc > 0.0) ){
		exeenv.AntXc += double(X - (PBoxAnt->Width / 2)) * ((1.0 / newSc) - (1.0 / oldSc));
		exeenv.AntYc += double(Y - (PBoxAnt->Height / 2)) * ((1.0 / newSc) - (1.0 / oldSc));
	}
	PBoxAnt->Invalidate();
}
//---------------------------------------------------------------------------
// アンテナ形状表示のマウスイベント
void __fastcall TMainWnd::PBoxAntMouseDown(TObject *Sender, TMouseButton Button,
	TShiftState Shift, int X, int Y)
{
	PBoxAntMX = X;
	PBoxAntMY = Y;
	PBoxAntClickCtrl = Shift.Contains(ssCtrl) ? TRUE : FALSE;
	if( (Button == mbLeft) && BeginAntGizmoDrag(X, Y) == TRUE ){
		PBoxAntDragButton = -1;
		PBoxAntDragAction = ANT_MOUSE_NONE;
		return;
	}
	if( AntDrawMode ){
		PBoxAntIgnoreClick = TRUE;
		PBoxAntDragButton = -1;
		PBoxAntDragAction = ANT_MOUSE_NONE;
		if( Button == mbRight ){
			AntDrawActive = FALSE;
			AntDrawSnapVisible = FALSE;
			PBoxAnt->Invalidate();
			return;
		}
		if( Button != mbLeft ) return;
		double wx, wy, wz;
		if( AntViewPointToXY(X, Y, wx, wy, wz) != TRUE ){
			::MessageBeep(MB_ICONEXCLAMATION);
			return;
		}
		SnapAntDrawPoint(X, Y, wx, wy, wz);
		if( !AntDrawActive ){
			AntDrawX1 = AntDrawX2 = wx;
			AntDrawY1 = AntDrawY2 = wy;
			AntDrawZ1 = AntDrawZ2 = wz;
			AntDrawActive = TRUE;
			PBoxAnt->Invalidate();
		}
		else {
			AntDrawX2 = wx;
			AntDrawY2 = wy;
			AntDrawZ2 = wz;
			AddAntDrawWire(AntDrawX1, AntDrawY1, AntDrawZ1, AntDrawX2, AntDrawY2, AntDrawZ2);
			AntDrawActive = FALSE;
		}
		return;
	}
	PBoxAntLastX = X;
	PBoxAntLastY = Y;
	PBoxAntStartDeg = TBarDeg->Position;
	PBoxAntStartZDeg = TBarZDeg->Position;
	PBoxAntDragButton = int(Button);
	PBoxAntDragAction = GetAntMouseAction(Button);
	PBoxAntDragMoved = FALSE;
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::PBoxAntMouseMove(TObject *Sender, TShiftState Shift, int X, int Y)
{
	if( AntGizmoDrag ){
		UpdateAntGizmoDrag(X, Y, Shift);
		return;
	}
	if( AntDrawMode ){
		double wx, wy, wz;
		if( AntViewPointToXY(X, Y, wx, wy, wz) == TRUE ){
			SnapAntDrawPoint(X, Y, wx, wy, wz);
			if( AntDrawActive ){
				AntDrawX2 = wx;
				AntDrawY2 = wy;
				AntDrawZ2 = wz;
			}
		}
		else {
			AntDrawSnapVisible = FALSE;
		}
		PBoxAnt->Invalidate();
		return;
	}
	if( PBoxAntDragAction == ANT_MOUSE_NONE ) return;
	int dx = X - PBoxAntLastX;
	int dy = Y - PBoxAntLastY;
	if( !dx && !dy ) return;
	if( !PBoxAntDragMoved && (ABS(X - PBoxAntMX) < 3) && (ABS(Y - PBoxAntMY) < 3) ) return;
	PBoxAntDragMoved = TRUE;
	switch( PBoxAntDragAction ){
		case ANT_MOUSE_PAN:
			AntViewPan(dx, dy);
			break;
		case ANT_MOUSE_ROTATE:
			AntViewRotateDrag(X, Y);
			break;
		case ANT_MOUSE_ZOOM:
			AntViewZoom(-dy / 2, X, Y);
			break;
	}
	PBoxAntLastX = X;
	PBoxAntLastY = Y;
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::PBoxAntMouseUp(TObject *Sender, TMouseButton Button,
	TShiftState Shift, int X, int Y)
{
	if( AntGizmoDrag ){
		EndAntGizmoDrag();
		return;
	}
	if( AntDrawMode ){
		PBoxAntDragButton = -1;
		PBoxAntDragAction = ANT_MOUSE_NONE;
		return;
	}
	if( int(Button) == PBoxAntDragButton ){
		PBoxAntDragButton = -1;
		PBoxAntDragAction = ANT_MOUSE_NONE;
		if( PBoxAntDragMoved ) PBoxAntIgnoreClick = TRUE;
	}
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::PBoxAntMouseWheel(TObject *Sender, TShiftState Shift,
	int WheelDelta, const TPoint &MousePos, bool &Handled)
{
	TPoint pt = PBoxAnt->ScreenToClient(MousePos);
	if( (pt.x < 0) || (pt.y < 0) || (pt.x >= PBoxAnt->Width) || (pt.y >= PBoxAnt->Height) ) return;
	Handled = TRUE;
	int step = WheelDelta / WHEEL_DELTA;
	if( !step ) step = (WheelDelta > 0) ? 1 : -1;
	switch( exeenv.AntMouseWheel ){
		case ANT_MOUSE_PAN:
			AntViewPan(0, step * 24);
			break;
		case ANT_MOUSE_ROTATE:
			AntViewRotate(step * 5, 0);
			break;
		case ANT_MOUSE_ZOOM:
			AntViewZoom(step * 2, pt.x, pt.y);
			break;
	}
}
//---------------------------------------------------------------------------
// アンテナ形状表示のシングルクリック
void __fastcall TMainWnd::PBoxAntClick(TObject *Sender)
{
	if( PBoxAntIgnoreClick ){
		PBoxAntIgnoreClick = FALSE;
		return;
	}
	if( SelectWire(PBoxAntMX, PBoxAntMY) == TRUE ){
		PBoxAnt->Invalidate();
	}
	else {
		DeselectAntWireSelection();
		Grid2->EditorMode = FALSE;
		Grid2->Invalidate();
		PBoxAnt->Invalidate();
	}
}
//---------------------------------------------------------------------------
// アンテナ形状表示のダブルクリック
void __fastcall TMainWnd::PBoxAntDblClick(TObject *Sender)
{
	if( AntDrawMode ) return;
	int w = Grid2->Row - 1;
	if( !exeenv.CalcF && (GetAntSelectionCount() > 0) && (w >= 0) && (w < ant.wmax) && (KT1->Enabled == TRUE) ){
		KT1Click(NULL);
	}
}
//---------------------------------------------------------------------------
// ポップアップ－クリック位置を中心にする
void __fastcall TMainWnd::K29Click(TObject *Sender)
{
	double sc = double(TBarSC->Position) / 20.0;
	sc = sc * sc * sc * sc;
	exeenv.AntXc += double((PBoxAnt->Width/2) - PBoxAntMX)/sc;
	exeenv.AntYc += double((PBoxAnt->Height/2) - PBoxAntMY)/sc;
	PBoxAnt->Invalidate();
}
//---------------------------------------------------------------------------
// アンテナ形状表示の原点中心ボタン
void __fastcall TMainWnd::OrgBtnClick(TObject *Sender)
{
	exeenv.AntXc = exeenv.AntYc = 0.0;
	PBoxAnt->Invalidate();
}
//---------------------------------------------------------------------------
// アンテナ形状表示 全景ボタン
void __fastcall TMainWnd::AllViewBtnClick(TObject *Sender)
{
	double	XL, XH, YL, YH, D, x, y;
	int		i, pos;

	if( !ant.wmax ) return;
	double deg = double(TBarDeg->Position);
	deg *= (PAI / 180.0);
	double zdeg = double(TBarZDeg->Position);
	zdeg *= (PAI / 180.0);
	XL = MAXDOUBLE; XH = -MAXDOUBLE;
	YL = MAXDOUBLE; YH = -MAXDOUBLE;
	pos = 0;
	for( i = 0; i < pCalAnt->wmax; i++ ){
		CalcAntViewXY(x, y, deg, zdeg, pCalAnt->wdef[i].X1, pCalAnt->wdef[i].Y1, pCalAnt->wdef[i].Z1);
		if( XL > x ) XL = x;
		if( XH < x ) XH = x;
		if( YL > y ) YL = y;
		if( YH < y ) YH = y;
		CalcAntViewXY(x, y, deg, zdeg, pCalAnt->wdef[i].X2, pCalAnt->wdef[i].Y2, pCalAnt->wdef[i].Z2);
		if( XL > x ) XL = x;
		if( XH < x ) XH = x;
		if( YL > y ) YL = y;
		if( YH < y ) YH = y;
		pos = 1;
	}
	if( pos ){
		exeenv.AntXc = -(XL + XH)/2;
		exeenv.AntYc = (YL + YH)/2;
		D = ABS(XH - XL);
		if( D < ABS(YH - YL) ) D = ABS(YH - YL);
		D *= 1.2;
		if( D > 0.0 ){
			D = (PBoxAnt->Height)/D;
			pos = int(sqrt(sqrt(D)) * 20.0);
			if( pos > TBarSC->Max ){
				pos = TBarSC->Max;
			}
		}
		else {
			pos = TBarSC->Max;
		}
	}
	else {
		exeenv.AntXc = exeenv.AntYc = 0;
		pos = 50;
	}
	TBarSC->Position = pos;
//	AntSC = double(TBarSC->Position) / 20.0;
//	AntSC = AntSC * AntSC * AntSC * AntSC;
//	ScrXc = int(tp->Width/2 + (AntXc * AntSC));
//	ScrYc = int(tp->Height/2 + (AntYc * AntSC));
#if 0
	D = ABS(XL);
	if( D < ABS(XH) ) D = ABS(XH);
	if( D < ABS(YL) ) D = ABS(YL);
	if( D < ABS(YH) ) D = ABS(YH);
	D *= 1.1;
	if( D > 0.0 ){
		D = (PBoxAnt->Height/2) / D;
		pos = int(sqrt(sqrt(D)) * 20.0);
		if( pos > TBarSC->Max ){
			pos = TBarSC->Max;
		}
	}
	else {
		pos = TBarSC->Max;
	}
	TBarSC->Position = pos;
	exeenv.AntXc = exeenv.AntYc = 0.0;
#endif
	PBoxAnt->Invalidate();
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::TBarDegChange(TObject *Sender)
{
	PBoxAnt->Invalidate();
}
