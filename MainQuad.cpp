//Copyright+LGPL
#include <vcl.h>
#pragma hdrstop

#include "Main.h"

//---------------------------------------------------------------------------
// Initialize quad-split layout panels
void __fastcall TMainWnd::InitQuadLayout(void)
{
	QuadContainer = new TPanel(this);
	QuadContainer->Parent = this;
	QuadContainer->BevelOuter = bvNone;
	QuadContainer->Align = alNone;
	QuadContainer->Color = clBtnFace;
	QuadContainer->Visible = false;

	PanelTop = new TPanel(QuadContainer);
	PanelTop->Parent = QuadContainer;
	PanelTop->BevelOuter = bvNone;
	PanelTop->Align = alTop;
	PanelTop->Height = 300;

	SplitterH = new TSplitter(QuadContainer);
	SplitterH->Parent = QuadContainer;
	SplitterH->Align = alTop;
	SplitterH->Height = 4;
	SplitterH->ResizeStyle = rsUpdate;

	PanelBottom = new TPanel(QuadContainer);
	PanelBottom->Parent = QuadContainer;
	PanelBottom->BevelOuter = bvNone;
	PanelBottom->Align = alClient;

	PanelTopLeft = new TPanel(PanelTop);
	PanelTopLeft->Parent = PanelTop;
	PanelTopLeft->BevelOuter = bvLowered;
	PanelTopLeft->Align = alLeft;
	PanelTopLeft->Width = 300;
	PanelTopLeft->Caption = "";

	SplitterVTop = new TSplitter(PanelTop);
	SplitterVTop->Parent = PanelTop;
	SplitterVTop->Align = alLeft;
	SplitterVTop->Width = 4;
	SplitterVTop->ResizeStyle = rsUpdate;

	PanelTopRight = new TPanel(PanelTop);
	PanelTopRight->Parent = PanelTop;
	PanelTopRight->BevelOuter = bvLowered;
	PanelTopRight->Align = alClient;
	PanelTopRight->Caption = "";

	PanelBottomLeft = new TPanel(PanelBottom);
	PanelBottomLeft->Parent = PanelBottom;
	PanelBottomLeft->BevelOuter = bvLowered;
	PanelBottomLeft->Align = alLeft;
	PanelBottomLeft->Width = 300;
	PanelBottomLeft->Caption = "";

	SplitterVBot = new TSplitter(PanelBottom);
	SplitterVBot->Parent = PanelBottom;
	SplitterVBot->Align = alLeft;
	SplitterVBot->Width = 4;
	SplitterVBot->ResizeStyle = rsUpdate;

	PanelBottomRight = new TPanel(PanelBottom);
	PanelBottomRight->Parent = PanelBottom;
	PanelBottomRight->BevelOuter = bvLowered;
	PanelBottomRight->Align = alClient;
	PanelBottomRight->Caption = "";

	TMenuItem *KQ1 = new TMenuItem(MainMenu);
	KQ1->Caption = "Quad View(&Q)";
	KQ1->OnClick = QuadViewToggle;
	KQ1->ShortCut = Vcl::Menus::ShortCut('Q', TShiftState() << ssCtrl);
	KV1->Insert(0, KQ1);
	KAntDrawWire = new TMenuItem(MainMenu);
	KAntDrawWire->Caption = "3D Wire Draw(&D)";
	KAntDrawWire->AutoCheck = false;
	KAntDrawWire->OnClick = AntDrawWireToggle;
	KAntDrawWire->ShortCut = Vcl::Menus::ShortCut('D', TShiftState() << ssCtrl);
	KV1->Insert(1, KAntDrawWire);
	TMenuItem *NQ = new TMenuItem(MainMenu);
	NQ->Caption = "-";
	KV1->Insert(2, NQ);
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::LayoutQuadAntPanel(void)
{
	const int margin = 4;
	int cw = PanelTopRight->ClientWidth;
	int ch = PanelTopRight->ClientHeight;

	LayoutAntDrawControls();

	Label7->Font->Assign(Font);
	Label8->Font->Assign(Font);
	Label13->Font->Assign(Font);
	Label17->Font->Assign(Font);
	Ant3D->Font->Assign(Font);
	DspCur->Font->Assign(Font);
	DspPlus->Font->Assign(Font);

	int labelH = Label7->Height;
	if( Label8->Height > labelH ) labelH = Label8->Height;
	if( Label13->Height > labelH ) labelH = Label13->Height;
	if( Label17->Height > labelH ) labelH = Label17->Height;
	if( Ant3D->Height > labelH ) labelH = Ant3D->Height;

	int barH = TBarDeg->Height;
	int controlTop = ch - labelH - barH - (margin * 2);
	int pboxTop = 33;
	if( controlTop < pboxTop + 32 ) controlTop = pboxTop + 32;
	if( controlTop > ch - 32 ) controlTop = ch - 32;

	PBoxAnt->Align = alNone;
	PBoxAnt->SetBounds(0, pboxTop, cw, controlTop - pboxTop - margin);

	int labelTop = controlTop;
	int barTop = labelTop + labelH + 2;
	int trackW = (cw - 96) / 5;
	if( trackW < 90 ) trackW = 90;
	if( trackW > 180 ) trackW = 180;

	int x1 = margin + 6;
	int x2 = x1 + trackW + 28;
	int x3 = x2 + trackW + 40;
	int curX = cw - trackW - margin - 10;
	int checkX = x3 + trackW + 26;
	if( checkX + 130 > curX ) checkX = curX - 130;
	if( checkX < x3 + trackW + 8 ) checkX = x3 + trackW + 8;

	Label7->SetBounds(x1 + (trackW - Label7->Width) / 2, labelTop, Label7->Width, Label7->Height);
	TBarDeg->SetBounds(x1, barTop, trackW, barH);
	Label17->SetBounds(x2 + (trackW - Label17->Width) / 2, labelTop, Label17->Width, Label17->Height);
	TBarZDeg->SetBounds(x2, barTop, trackW, barH);
	Label8->SetBounds(x3 + (trackW - Label8->Width) / 2, labelTop, Label8->Width, Label8->Height);
	TBarSC->SetBounds(x3, barTop, trackW, barH);
	Ant3D->SetBounds(checkX, labelTop, Ant3D->Width, Ant3D->Height);
	DspCur->SetBounds(checkX, barTop, DspCur->Width, DspCur->Height);
	DspPlus->SetBounds(checkX, barTop + DspCur->Height + 2, DspPlus->Width, DspPlus->Height);
	Label13->SetBounds(curX + (trackW - Label13->Width) / 2, labelTop, Label13->Width, Label13->Height);
	TBarCur->SetBounds(curX, barTop, trackW, barH);
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::LayoutQuadPtnPanel(void)
{
	if( !QuadMode ) return;

	int top = 0;
	int bottom = PanelBottomRight->ClientHeight;

	if( AntName4->Visible ){
		int b = AntName4->Top + AntName4->Height + 4;
		if( b > top ) top = b;
	}
	if( PrintBtn->Visible ){
		int b = PrintBtn->Top + PrintBtn->Height + 4;
		if( b > top ) top = b;
	}
	if( WaveSel->Visible ){
		int t = WaveSel->Top - 4;
		if( t < bottom ) bottom = t;
	}
	if( EleBtn->Visible ){
		int t = EleBtn->Top - 4;
		if( t < bottom ) bottom = t;
	}
	if( bottom < top + 32 ) bottom = top + 32;
	if( bottom > PanelBottomRight->ClientHeight ) bottom = PanelBottomRight->ClientHeight;

	PBoxPtn->Align = alNone;
	PBoxPtn->SetBounds(0, top, PanelBottomRight->ClientWidth, bottom - top);
}
//---------------------------------------------------------------------------
void __fastcall TMainWnd::LayoutQuadPanels(void)
{
	if( !QuadMode ) return;

	PBoxAnt->Align = alNone;
	PBoxPtn->Align = alNone;

	AlignList.NewAlign(PanelTopLeft);
	AlignGrid[0].NewAlign(Grid1);
	AlignGrid[1].NewAlign(Grid2);
	AlignGrid[2].NewAlign(Grid3);
	AlignGrid[3].NewAlign(Grid4);

	LayoutQuadAntPanel();
	LayoutQuadPtnPanel();
}
//---------------------------------------------------------------------------
// Switch to quad-split view
void __fastcall TMainWnd::SwitchToQuadMode(void)
{
	if( QuadMode ) return;

	QuadSavedClientWidth = ClientWidth;
	QuadSavedClientHeight = ClientHeight;
	QuadSwitching = true;
	QuadMode = true;

	const int PANE_W = 792;
	const int PANE_H = 545;
	const int SPLIT  = 4;
	ClientWidth  = PANE_W * 2 + SPLIT;
	ClientHeight = PANE_H * 2 + SPLIT;
	PanelTop->Height       = PANE_H;
	PanelTopLeft->Width    = PANE_W;
	PanelBottomLeft->Width = PANE_W;

	while( TabSheet1->ControlCount > 0 )
		TabSheet1->Controls[0]->Parent = PanelTopLeft;

	while( TabSheet2->ControlCount > 0 )
		TabSheet2->Controls[0]->Parent = PanelTopRight;
	LayoutQuadAntPanel();

	while( TabSheet3->ControlCount > 0 )
		TabSheet3->Controls[0]->Parent = PanelBottomLeft;

	while( TabSheet5->ControlCount > 0 )
		TabSheet5->Controls[0]->Parent = PanelBottomRight;

	Page->Align = alNone;
	Page->Visible = false;
	QuadContainer->SetBounds(0, 0, ClientWidth, ClientHeight);
	QuadContainer->Align = alClient;
	QuadContainer->Visible = true;
	QuadSwitching = false;
	LayoutQuadPanels();

	DrawPtnH.SetRect(PBoxPtn->Canvas, 0, 0, PBoxPtn->Width/2-2, PBoxPtn->Height);
	DrawPtnV.SetRect(PBoxPtn->Canvas, PBoxPtn->Width/2+2, 0, PBoxPtn->Width, PBoxPtn->Height);
	PBoxAnt->Invalidate();
	PBoxPtn->Invalidate();
}
//---------------------------------------------------------------------------
// Switch back to tab view
void __fastcall TMainWnd::SwitchToTabMode(void)
{
	if( !QuadMode ) return;
	QuadSwitching = true;

	PBoxAnt->Align = alNone;
	PBoxPtn->Align = alNone;

	while( PanelBottomRight->ControlCount > 0 )
		PanelBottomRight->Controls[0]->Parent = TabSheet5;

	while( PanelBottomLeft->ControlCount > 0 )
		PanelBottomLeft->Controls[0]->Parent = TabSheet3;

	while( PanelTopRight->ControlCount > 0 )
		PanelTopRight->Controls[0]->Parent = TabSheet2;

	while( PanelTopLeft->ControlCount > 0 )
		PanelTopLeft->Controls[0]->Parent = TabSheet1;

	QuadContainer->Align = alNone;
	QuadContainer->Visible = false;
	Page->Align = alClient;
	Page->Visible = true;
	if( (QuadSavedClientWidth > 0) && (QuadSavedClientHeight > 0) ){
		ClientWidth = QuadSavedClientWidth;
		ClientHeight = QuadSavedClientHeight;
	}
	QuadMode = false;
	QuadSwitching = false;

	AlignList.NewAlign(Page->ActivePage);
	AlignGrid[0].NewAlign(Grid1);
	AlignGrid[1].NewAlign(Grid2);
	AlignGrid[2].NewAlign(Grid3);
	AlignGrid[3].NewAlign(Grid4);
	LayoutAntDrawControls();
	DrawPtnH.SetRect(PBoxPtn->Canvas, 0, 0, PBoxPtn->Width/2-2, PBoxPtn->Height);
	DrawPtnV.SetRect(PBoxPtn->Canvas, PBoxPtn->Width/2+2, 0, PBoxPtn->Width, PBoxPtn->Height);
	UpdateAllViews();
}
//---------------------------------------------------------------------------
// Toggle between quad-split and tab view
void __fastcall TMainWnd::QuadViewToggle(TObject *Sender)
{
	if( QuadMode )
		SwitchToTabMode();
	else
		SwitchToQuadMode();
}
