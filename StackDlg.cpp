//Copyright+LGPL

//-----------------------------------------------------------------------------------------------------------------------------------------------
// Copyright 1999-2013 Makoto Mori, Nobuyuki Oba
//-----------------------------------------------------------------------------------------------------------------------------------------------
// This file is part of MMANA.

// MMANA is free software: you can redistribute it and/or modify it under the terms of the GNU Lesser General Public License
// as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.

// MMANA is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU Lesser General Public License for more details.

// You should have received a copy of the GNU Lesser General Public License along with MMANA.  If not, see
// <http://www.gnu.org/licenses/>.
//-----------------------------------------------------------------------------------------------------------------------------------------------

//---------------------------------------------------------------------
#include <vcl.h>
#pragma hdrstop

#include "StackDlg.h"
//---------------------------------------------------------------------
#pragma resource "*.dfm"
//TStackDlgBox *StackDlgBox;
//---------------------------------------------------------------------
__fastcall TStackDlgBox::TStackDlgBox(TComponent* AOwner)
	: TForm(AOwner)
{
	pAnt = NULL;

	const BYTE VCTbl[]={1, 2, 4, 8, 0};

	char	bf[32];
	for( int i = 0; VCTbl[i]; i++ ){
		sprintf(bf, "%d", VCTbl[i]);
		VC->Items->Add(bf);
	}
	HC->Items->Assign(VC->Items);
	CBSame->Checked = FALSE;
	ChgEnb = 0;
}
//---------------------------------------------------------------------
void __fastcall TStackDlgBox::UpdateView(void)
{
	if( RmdSel ){
		LVW->Caption = "λ";
		LHW->Caption = LVW->Caption;
	}
	else {
		LVW->Caption = "ｍ";
		LHW->Caption = LVW->Caption;
	}
	UpdateAntBtn();
}
//---------------------------------------------------------------------
void __fastcall TStackDlgBox::UpdateAntBtn(void)
{
	int vc, hc;

	CalcU(vc, AnsiString(VC->Text).c_str());
	CalcU(hc, AnsiString(HC->Text).c_str());
	AntNewBtn->Enabled = (vc * hc != 1) ? TRUE : FALSE;
}
//---------------------------------------------------------------------
int __fastcall TStackDlgBox::Execute(ANTDEF *ap)
{
	int		vc, hc;

	ChgEnb = 0;
	pAnt = ap;
	StackVW = ap->StackVW;
	StackHW = ap->StackHW;
	RMD = 299.8/ap->fq;
	VC->Text = ap->StackV;
	HC->Text = ap->StackH;
	CalcU(vc, AnsiString(VC->Text).c_str());
	CalcU(hc, AnsiString(HC->Text).c_str());
	RmdSel = 1;
	VW->Text = StrDbl(StackVW/RMD);
	HW->Text = StrDbl(StackHW/RMD);
	if ( (hc * vc != 1) && (StackVW == StackHW) ) CBSame->Checked = TRUE;
	VT->ItemIndex = ap->StackVT;
	UpdateView();
	CBRmdSel->Checked = RmdSel;
	while(1){
		ChgEnb = 1;
		int r = ShowModal();
		if( r != IDCANCEL ){
			double	vvw, hhw;
			SetWireW(vvw, hhw);
			if( CalcU(vc, AnsiString(VC->Text).c_str())==FALSE ) continue;
			if( CalcU(hc, AnsiString(HC->Text).c_str())==FALSE ) continue;
			if( (vc < 1) || (hc < 1) ) continue;
            if( (vc * hc * ap->wmax) > WMAX ){
				ErrorMB("ワイヤの数が多すぎます.");
                continue;
            }
            if( (vc * hc * ap->cmax) > CMAX ){
				ErrorMB("給電点の数が多すぎます.");
                continue;
            }
            if( (vc * hc * ap->lmax) > LMAX ){
				ErrorMB("集中定数の数が多すぎます.");
                continue;
            }
			if( (StackVW <= 0.0) || (StackHW <= 0.0) ) continue;
            if( ((vc>1)&&(StackVW <= vvw)) || ((hc>1)&&(StackHW <= hhw)) ){
				ErrorMB("間隔が狭すぎて展開したワイヤが重なります.");
                continue;
            }
			if( r == 1024 ){
				if( (vc * hc) != 1 ){
					if( YesNoMB("アンテナ定義が変更されます(元の状態に戻せなくなります).\r\n\r\nほんまに変更しますか？") != IDYES ) continue;
                }
                else {
					r = IDOK;
                }
            }
			ap->StackH = hc;
			ap->StackV = vc;
			ap->StackHW = StackHW;
			ap->StackVW = StackVW;
            ap->StackVT = VT->ItemIndex;
			return (r != IDOK) ? r : TRUE;
	    }
        else {
			break;
        }
    }
    return FALSE;
}
//---------------------------------------------------------------------
void __fastcall TStackDlgBox::SetWireW(double &vw, double &hw)
{
	double	ymax = -MAXDOUBLE;
    double	ymin = MAXDOUBLE;
    double	zmax = -MAXDOUBLE;
    double	zmin = MAXDOUBLE;
	int	i;
	for( i = 0; i < pAnt->wmax; i++ ){
		if( ymax < pAnt->wdef[i].Y1 ) ymax = pAnt->wdef[i].Y1;
		if( ymax < pAnt->wdef[i].Y2 ) ymax = pAnt->wdef[i].Y2;
		if( zmax < pAnt->wdef[i].Z1 ) zmax = pAnt->wdef[i].Z1;
		if( zmax < pAnt->wdef[i].Z2 ) zmax = pAnt->wdef[i].Z2;
		if( ymin > pAnt->wdef[i].Y1 ) ymin = pAnt->wdef[i].Y1;
		if( ymin > pAnt->wdef[i].Y2 ) ymin = pAnt->wdef[i].Y2;
		if( zmin > pAnt->wdef[i].Z1 ) zmin = pAnt->wdef[i].Z1;
		if( zmin > pAnt->wdef[i].Z2 ) zmin = pAnt->wdef[i].Z2;
    }
    vw = zmax - zmin;
    hw = ymax - ymin;
}
//---------------------------------------------------------------------
void __fastcall TStackDlgBox::CBRmdSelClick(TObject *Sender)
{
	if( RmdSel != CBRmdSel->Checked ){
		RmdSel = CBRmdSel->Checked;
		double	vw = StackVW;
        double	hw = StackHW;
		ChgEnb = 0;
		if( RmdSel ){
			vw /= RMD;
            hw /= RMD;
        }
        VW->Text = StrDbl(vw);
        HW->Text = StrDbl(hw);
        ChgEnb = 1;
		UpdateView();
		HW->SetFocus();
    }
}
//---------------------------------------------------------------------------
void __fastcall TStackDlgBox::CBSameClick(TObject *Sender)
{
	if( !ChgEnb ) return;

	if( CBSame->Checked ){
		ChgEnb = 0;
		VW->Text = HW->Text;
        StackVW = StackHW;
		ChgEnb = 1;
    }
}
//---------------------------------------------------------------------------
void __fastcall TStackDlgBox::HWChange(TObject *Sender)
{
	if( !ChgEnb ) return;

	Calc(StackHW, AnsiString(HW->Text).c_str());
	if( RmdSel ) StackHW *= RMD;
	if( CBSame->Checked ){
		ChgEnb = 0;
		StackVW = StackHW;
		VW->Text = HW->Text;
		ChgEnb = 1;
    }
}
//---------------------------------------------------------------------------
void __fastcall TStackDlgBox::VWChange(TObject *Sender)
{
	if( !ChgEnb ) return;

	Calc(StackVW, AnsiString(VW->Text).c_str());
	if( RmdSel ) StackVW *= RMD;
	if( CBSame->Checked ){
		ChgEnb = 0;
		StackHW = StackVW;
		HW->Text = VW->Text;
		ChgEnb = 1;
    }
}
//---------------------------------------------------------------------------
void __fastcall TStackDlgBox::HCChange(TObject *Sender)
{
	UpdateAntBtn();
}
//---------------------------------------------------------------------------
void __fastcall TStackDlgBox::VCChange(TObject *Sender)
{
	UpdateAntBtn();
}
//---------------------------------------------------------------------------
