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

#include "ACalEle.h"
//---------------------------------------------------------------------
#pragma resource "*.dfm"
//TACalEleBox *ACalEleBox;
//---------------------------------------------------------------------
__fastcall TACalEleBox::TACalEleBox(TComponent* AOwner)
	: TForm(AOwner)
{
}
//---------------------------------------------------------------------
void __fastcall TACalEleBox::UpdateItemIndex(void)
{
    RGHW->Items->Strings[1] = CBVW->Checked ? "縦幅に連携" : "OFF";
	RGHW->Items->Strings[2] = CBXW->Checked ? "Ｘ幅に連携" : "OFF";
    RGVW->Items->Strings[1] = CBHW->Checked ? "横幅に連携" : "OFF";
	RGVW->Items->Strings[2] = CBXW->Checked ? "Ｘ幅に連携" : "OFF";
    RGXW->Items->Strings[1] = CBHW->Checked ? "横幅に連携" : "OFF";
	RGXW->Items->Strings[2] = CBVW->Checked ? "縦幅に連携" : "OFF";
}
//---------------------------------------------------------------------
int __fastcall TACalEleBox::Execute(int &HW, int &VW, int &XW, int &HC, int &VC, int &XC)
{
	CBHW->Checked = HW;
	if( !HW ){
		CBHW->Enabled = FALSE;
        RGHW->Enabled = FALSE;
    }
    CBVW->Checked = VW;
	if( !VW ){
		CBVW->Enabled = FALSE;
        RGVW->Enabled = FALSE;
    }
    CBXW->Checked = XW;
	if( !XW ){
		CBXW->Enabled = FALSE;
        RGXW->Enabled = FALSE;
    }
    RGHW->Items->Strings[1] = VW ? "縦幅に連携" : "OFF";
	RGHW->Items->Strings[2] = XW ? "Ｘ幅に連携" : "OFF";
    RGVW->Items->Strings[1] = HW ? "横幅に連携" : "OFF";
	RGVW->Items->Strings[2] = XW ? "Ｘ幅に連携" : "OFF";
    RGXW->Items->Strings[1] = HW ? "横幅に連携" : "OFF";
	RGXW->Items->Strings[2] = VW ? "縦幅に連携" : "OFF";
    RGHW->ItemIndex = 0;
    RGVW->ItemIndex = 0;
    RGXW->ItemIndex = 0;
	ShowModal();
	HW = CBHW->Checked;
	VW = CBVW->Checked;
	XW = CBXW->Checked;
    HC = RGHW->ItemIndex;
    VC = RGVW->ItemIndex;
    XC = RGXW->ItemIndex;
    return TRUE;
}
//---------------------------------------------------------------------
void __fastcall TACalEleBox::CBHWClick(TObject *Sender)
{
	UpdateItemIndex();	
}
//---------------------------------------------------------------------------
