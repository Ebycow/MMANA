//Copyright+LGPL
#include <vcl.h>
#pragma hdrstop

#include "Main.h"

static const char WIRE_NOTE_BEGIN[] = "[MMANA-WIRE-NOTES]";
static const char WIRE_NOTE_END[] = "[/MMANA-WIRE-NOTES]";

static int HexVal(int c)
{
	if( (c >= '0') && (c <= '9') ) return c - '0';
	if( (c >= 'A') && (c <= 'F') ) return c - 'A' + 10;
	if( (c >= 'a') && (c <= 'f') ) return c - 'a' + 10;
	return -1;
}

static void AppendHexByte(AnsiString &out, unsigned char c)
{
	static const char hx[] = "0123456789ABCDEF";
	char bf[4];
	bf[0] = '%';
	bf[1] = hx[(c >> 4) & 15];
	bf[2] = hx[c & 15];
	bf[3] = 0;
	out += bf;
}

static AnsiString EncodeWireNote(LPCSTR p)
{
	AnsiString out;
	for( ; *p; p++ ){
		unsigned char c = (unsigned char)*p;
		if( ((c >= 'A') && (c <= 'Z')) || ((c >= 'a') && (c <= 'z')) ||
			((c >= '0') && (c <= '9')) || (c == ' ') || (c == '.') ||
			(c == '_') || (c == '-') || (c == '/') || (c == ':') ||
			(c == '(') || (c == ')') || (c == '[') || (c == ']') ){
			char bf[2];
			bf[0] = c;
			bf[1] = 0;
			out += bf;
		}
		else {
			AppendHexByte(out, c);
		}
	}
	return out;
}

static void DecodeWireNote(LPSTR out, int outSize, LPCSTR p)
{
	int n = 0;
	if( outSize <= 0 ) return;
	while( *p && (n < (outSize - 1)) ){
		if( (p[0] == '%') && p[1] && p[2] ){
			int h1 = HexVal((unsigned char)p[1]);
			int h2 = HexVal((unsigned char)p[2]);
			if( (h1 >= 0) && (h2 >= 0) ){
				out[n++] = char((h1 << 4) | h2);
				p += 3;
				continue;
			}
		}
		out[n++] = *p++;
	}
	out[n] = 0;
}

void StripWireNotes(ANTDEF *ap, AnsiString &rem)
{
	if( ap != NULL ){
		for( int i = 0; i < WMAX; i++ ) ap->wdef[i].NOTE[0] = 0;
	}

	CTextString cs(rem);
	AnsiString clean;
	char bf[1024];
	int inBlock = FALSE;
	while( cs.LoadText(bf, 1023) == TRUE ){
		if( !strcmp(bf, WIRE_NOTE_BEGIN) ){
			inBlock = TRUE;
			continue;
		}
		if( inBlock ){
			if( !strcmp(bf, WIRE_NOTE_END) ){
				inBlock = FALSE;
			}
			else if( ap != NULL ){
				char *eq = strchr(bf, '=');
				if( eq != NULL ){
					*eq++ = 0;
					int w = atoi(bf);
					if( (w > 0) && (w <= ap->wmax) ){
						DecodeWireNote(ap->wdef[w-1].NOTE, WNOTE_MAX, eq);
					}
				}
			}
			continue;
		}
		clean += bf;
		clean += "\r\n";
	}
	rem = clean;
}

void AppendWireNotes(ANTDEF *ap, AnsiString &rem)
{
	AnsiString notes;
	for( int i = 0; i < ap->wmax; i++ ){
		if( !ap->wdef[i].NOTE[0] ) continue;
		char bf[32];
		sprintf(bf, "%d=", i + 1);
		notes += bf;
		notes += EncodeWireNote(ap->wdef[i].NOTE);
		notes += "\r\n";
	}
	if( notes.IsEmpty() ) return;
	if( !rem.IsEmpty() && (LastC(rem.c_str()) != '\n') ) rem += "\r\n";
	rem += WIRE_NOTE_BEGIN;
	rem += "\r\n";
	rem += notes;
	rem += WIRE_NOTE_END;
	rem += "\r\n";
}

//---------------------------------------------------------------------------
//現在のアンテナ定義を文字列に変換
int __fastcall TMainWnd::SaveAntFile(LPCSTR pName)
{
	AnsiString	out;
	FILE		*fp;

	SaveAntStrings(out);
	if( (fp = fopen(pName, "wb"))!=NULL ){
		fputs(out.c_str(), fp);
		ant.Edit = 0;
		if( fclose(fp) ){
			ErrorMB("'%s'の作成に失敗しました.", pName);
			return FALSE;
		}
		else {
			return TRUE;
		}
	}
	else {
		ErrorMB("'%s'を作成できません.", pName);
		return FALSE;
	}
}
//---------------------------------------------------------------------------
//アンテナ定義ファイルを読込む
int __fastcall TMainWnd::LoadAntFile(LPCSTR pName)
{
	AnsiString	in;
	FILE		*fp;
	char		bf[1024];

	if( (fp = fopen(pName, "rt"))!=NULL ){
		while(!feof(fp)){
			if( fgets(bf, 1024, fp) != NULL ){
				ClipLF(bf);
				in += bf;
				in += "\r\n";
			}
		}
		fclose(fp);

		InitAntDef();
		LoadAntStrings(in);
		SetAntDef();
		exeenv.CalcLog = 1;
		SetStackAnt();
		res.ClearBWC();
		ant.Flag = 0;
		AllViewBtnClick(NULL);
		UpdateAllViews();
		ant.Edit = 0;
		return TRUE;
	}
	else {
		ErrorMB("'%s'が見つかりません.", pName);
		return FALSE;
	}
}

//---------------------------------------------------------------------------
//現在のアンテナ定義を文字列に変換
void __fastcall TMainWnd::SaveAntStrings(AnsiString &out)
{
	char	bf[256];
	int		i, j;
	LPSTR	t;

	GetAntDef();
	out = ant.Name;
	out += "\r\n*\r\n";
	sprintf(bf, "%s\r\n", StrDbl(ant.fq));
	out += bf;
	out += "*** ﾜｲﾔ ***\r\n";
	t = bf;
	if( (ant.StackH > 1)||(ant.StackV > 1) ){
		sprintf(t, "%d,\t%d,\t%d,\t", ant.wmax, ant.StackH, ant.StackV);t += strlen(t);
		sprintf(t, "%s,\t", StrDblE(ant.StackHW));t += strlen(t);
		sprintf(t, "%s,\t", StrDblE(ant.StackVW));t += strlen(t);
		sprintf(t, "%d\r\n", ant.StackVT);
	}
	else {
		sprintf(t, "%d\r\n", ant.wmax);
	}
	out += bf;
	for( i = 0; i < ant.wmax; i++ ){
		t = bf;
		sprintf(t, "%s,\t", StrDblE(ant.wdef[i].X1));t += strlen(t);
		sprintf(t, "%s,\t", StrDblE(ant.wdef[i].Y1));t += strlen(t);
		sprintf(t, "%s,\t", StrDblE(ant.wdef[i].Z1));t += strlen(t);
		sprintf(t, "%s,\t", StrDblE(ant.wdef[i].X2));t += strlen(t);
		sprintf(t, "%s,\t", StrDblE(ant.wdef[i].Y2));t += strlen(t);
		sprintf(t, "%s,\t", StrDblE(ant.wdef[i].Z2));t += strlen(t);
		sprintf(t, "%s,\t", StrDblE(ant.wdef[i].R));t += strlen(t);
		sprintf(t, "%s\r\n", Seg2Str(ant.wdef[i].SEG));
		out += bf;
	}
	out += "*** 給電点 ***\r\n";
	sprintf(bf, "%d,\t%d\r\n", ant.cmax, ant.cauto);
	out += bf;
	for( i = 0; i < ant.cmax; i++ ){
		t = bf;
		sprintf(t, "%s,\t", ant.cdef[i].PLUS);t += strlen(t);
		sprintf(t, "%s,\t", StrDblE(ant.cdef[i].DEG));t += strlen(t);
		sprintf(t, "%s\r\n", StrDblE(ant.cdef[i].EV));
		out += bf;
	}
	out += "*** 集中定数 ***\r\n";
	sprintf(bf, "%d,\t%d\r\n", ant.lmax, ant.lenb);
	out += bf;
	for( i = 0; i < ant.lmax; i++ ){
		t = bf;
		sprintf(t, "%s,\t", ant.ldef[i].PLUS);t += strlen(t);
		sprintf(t, "%d,\t", ant.ldef[i].TYPE);t += strlen(t);
		switch(ant.ldef[i].TYPE){
			case 0:		// L,C,Q
				sprintf(t, "%s,\t", StrDblE(ant.ldef[i].A[0]));t += strlen(t);
				sprintf(t, "%s,\t", StrDblE(ant.ldef[i].B[0]));t += strlen(t);
				sprintf(t, "%s\r\n", StrDblE(ant.ldef[i].A[1]));
				break;
			case 1:		// R+jX
				sprintf(t, "%s,\t", StrDblE(ant.ldef[i].A[0]));t += strlen(t);
				sprintf(t, "%s\r\n", StrDblE(ant.ldef[i].B[0]));
				break;
			case 2:		// S
				for( j = 0; j < ant.ldef[i].SN; j++ ){
					if( j ){
						*t++ = ',';
						*t++ = '\t';
					}
					sprintf(t, "%s,\t", StrDblE(ant.ldef[i].A[j]));t += strlen(t);
					sprintf(t, "%s", StrDblE(ant.ldef[i].B[j]));t += strlen(t);
				}
				strcpy(t, "\r\n");
				break;
		}
		out += bf;
	}
	t = bf;
	out += "*** 自動分割 ***\r\n";
	sprintf(t, "%d,\t%d,\t", ant.DM1, ant.DM2);t += strlen(t);
	sprintf(t, "%s,\t%d\r\n", StrDbl(ant.SC), ant.EC);
	out += bf;
	t = bf;
	out += "*** 計算環境 ***\r\n";
	sprintf(t, "%u,\t%s,\t%u,\t", env.type, StrDblE(env.antheight), env.WireRoss);
	t += strlen(t);
	sprintf(t, "%s,\t%u,\t%u,\t%.lf\r\n", StrDblE(env.RO), env.fbr & 0x0000ffff, (env.fbr >> 16), env.JXO );
	out += bf;
	if( ant.pmax ){
		out += "$$$ ワイヤ組み合わせデータ $$$\r\n";
		sprintf(bf, "%u\r\n", ant.pmax );
		out += bf;
		for( i = 0; i < ant.pmax; i++ ){
			t = bf;
			sprintf(t, "%s,\t", StrDblE(ant.pdef[i].RR)); t += strlen(t);
			sprintf(t, "%u,\t", ant.pdef[i].Type ); t += strlen(t);
			for( j = 0; ant.pdef[i].L[j] && (j < PPMAX); j++ ){
				if( j ){
					*t++ = ',';
					*t++ = '\t';
				}
				sprintf(t, "%s,\t", StrDblE(ant.pdef[i].L[j])); t += strlen(t);
				sprintf(t, "%s", StrDblE(ant.pdef[i].R[j])); t += strlen(t);
			}
			strcpy(t, "\r\n");
			out += bf;
		}
	}
	AnsiString rem = antRem;
	StripWireNotes(NULL, rem);
	AppendWireNotes(&ant, rem);
	if( *rem.c_str() ){
		out += "### コメント ###\r\n";
		out += rem;
	}
}

//---------------------------------------------------------------------------
//文字列からアンテナ定義を作成
void __fastcall TMainWnd::LoadAntStrings(AnsiString &in)
{
	CTextString	cs(in);
	int		i, j;
	char	bf[1024];
	LPSTR	t, p;

	antRem = "";
	if( cs.LoadTextLoop(bf, 1023) == FALSE ) goto _ex;
	strncpy(ant.Name, bf, sizeof(ant.Name));
	if( cs.LoadTextLoop(bf, 1023) == FALSE ) goto _ex;
	Calc(ant.fq, bf);
	if( cs.LoadTextLoop(bf, 1023) == FALSE ) goto _ex;
	p = StrDlm(t, bf);
	CalcU(ant.wmax, t);
	p = StrDlm(t, p);
	if( *t ){		// スタック設定
		CalcU(ant.StackH, t);
		p = StrDlm(t, p);
		CalcU(ant.StackV, t);
		p = StrDlm(t, p);
		Calc(ant.StackHW, t);
		p = StrDlm(t, p);
		Calc(ant.StackVW, t);
		StrDlm(t, p);
		CalcU(ant.StackVT, t);
		if( (ant.StackH < 1)||(ant.StackV < 1) ){
			ant.StackH = ant.StackV = 1;
		}
	}
	else {
		ant.StackVT = 0;
		ant.StackH = ant.StackV = 1;
	}
	if( (ant.StackH == 1) && (ant.StackV == 1) && ant.fq ){
		ant.StackHW = ant.StackVW = 299.8/ant.fq;
	}
	if( ant.wmax > WMAX ) ant.wmax = WMAX;
	for( i = 0; i < ant.wmax; i++ ){
		if( cs.LoadTextLoop(bf, 1023) == FALSE ) goto _ex;
		p = StrDlm(t, bf);	// X1
		Calc(ant.wdef[i].X1, t);
		p = StrDlm(t, p);	// Y1
		Calc(ant.wdef[i].Y1, t);
		p = StrDlm(t, p);	// Z1
		Calc(ant.wdef[i].Z1, t);
		p = StrDlm(t, p);	// X2
		Calc(ant.wdef[i].X2, t);
		p = StrDlm(t, p);	// Y2
		Calc(ant.wdef[i].Y2, t);
		p = StrDlm(t, p);	// Z2
		Calc(ant.wdef[i].Z2, t);
		p = StrDlm(t, p);	// R
		Calc(ant.wdef[i].R, t);
		p = StrDlm(t, p);	// SEG(DM1)
		CalcI(ant.wdef[i].SEG, t);
		StrDlm(t, p);		// SEG(DM2)
		if( *t ){
			CalcI(j, t);
			if( j ) ant.wdef[i].SEG = 0x80000000 | ABS(ant.wdef[i].SEG) | (ABS(j) << 16);
		}
	}
	if( cs.LoadTextLoop(bf, 1023) == FALSE ) goto _ex;
	p = StrDlm(t, bf);
	CalcU(ant.cmax, t);
	StrDlm(t, p);
	CalcU(ant.cauto, t); if( ant.cauto ) ant.cauto = 1;
	if( ant.cmax > CMAX ) ant.cmax = CMAX;
	for( i = 0; i < ant.cmax; i++ ){
		if( cs.LoadTextLoop(bf, 1023) == FALSE ) goto _ex;
		p = StrDlm(t, bf);	// PLUS
		ZSCopy(ant.cdef[i].PLUS, t, 8);
		p = StrDlm(t, p);	// DEG
		Calc(ant.cdef[i].DEG, t);
		StrDlm(t, p);		// EV
		Calc(ant.cdef[i].EV, t);
	}
	if( cs.LoadTextLoop(bf, 1023) == FALSE ) goto _ex;
	p = StrDlm(t, bf);
	CalcU(ant.lmax, t);
	StrDlm(t, p);
	CalcU(ant.lenb, t); if( ant.lenb ) ant.lenb = 1;
	if( ant.lmax > LMAX ) ant.lmax = LMAX;
	for( i = 0; i < ant.lmax; i++ ){
		if( cs.LoadTextLoop(bf, 1023) == FALSE ) goto _ex;
		p = StrDlm(t, bf);	// PLUS
		ZSCopy(ant.ldef[i].PLUS, t, 8);
		p = StrDlm(t, p);	// TYPE
		CalcU(ant.ldef[i].TYPE, t);
		switch(ant.ldef[i].TYPE){
			case 0:		// L,C,Q
				p = StrDlm(t, p);	// L
				Calc(ant.ldef[i].A[0], t);
				p = StrDlm(t, p);	// C
				Calc(ant.ldef[i].B[0], t);
				StrDlm(t, p);	// Q
				Calc(ant.ldef[i].A[1], t);
				break;
			case 1:		// R+jX
				p = StrDlm(t, p);	// R
				Calc(ant.ldef[i].A[0], t);
				StrDlm(t, p);	// jX
				Calc(ant.ldef[i].B[0], t);
				break;
			case 2:		// S
				for( j = 0; j < SMAX; j++ ){
					p = StrDlm(t, p);	// A[j]
					if( !*t ) break;
					Calc(ant.ldef[i].A[j], t);
					p = StrDlm(t, p);	// B[j]
					Calc(ant.ldef[i].B[j], t);
				}
				ant.ldef[i].SN = j;
				for( ; j < SMAX; j++ ){
					ant.ldef[i].A[j] = NULLV;
					ant.ldef[i].B[j] = NULLV;
				}
				break;
		}
	}

	if( cs.LoadTextLoop(bf, 1023) == TRUE ){
		p = StrDlm(t, bf);		// DM1
		CalcU(ant.DM1, t);
		p = StrDlm(t, p);		// DM2
		CalcU(ant.DM2, t);
		p = StrDlm(t, p);		// SC
		Calc(ant.SC, t);
		if( (ant.SC <= 1.0) || (ant.SC > 3.0) ){
			ant.SC = 2.0;
		}
		StrDlm(t, p);			// EC
		if( *t ){
			CalcU(ant.EC, t);
			if( ant.EC <= 0 ) ant.EC = 1;
		}
		else {
			ant.EC = 1;
		}
	}
	else {
		ant.DM2 = 400;
		ant.DM2 = 40;
		ant.EC = 1;
		ant.SC = 2.0;
	}

	if( cs.LoadTextLoop(bf, 1023) == FALSE ) goto _ex;
	p = StrDlm(t, bf);
	CalcU(env.type, t);
	if( env.type >= 3 ) env.type = 0;
	p = StrDlm(t, p);
	Calc(env.antheight, t);
	p = StrDlm(t, p);
	if( *t ){
		CalcU(env.WireRoss, t);
		if( (env.WireRoss < 0)||(env.WireRoss > 6) ){
			env.WireRoss = 0;
		}
		p = StrDlm(t, p);	// RO
		if( *t && (Calc(env.RO, t)==TRUE) ){
			p = StrDlm(t, p);
			CalcU(i, t);
			p = StrDlm(t, p);
			CalcU(j, t);
			env.fbr = i | (j << 16);
			StrDlm(t, p);
			if( *t ){
				Calc(env.JXO, t);
			}
		}
	}
	else {
		env.WireRoss = 0;
	}
	while(1){
		if( cs.LoadTextLoop(bf, 1023) == FALSE ) goto _ex;
		if( !strncmp(bf, "$$$", 3) ){
			if( cs.LoadTextLoop(bf, 1023) == FALSE ) goto _ex;
			CalcU(ant.pmax, bf);
			for( i = 0; (i < ant.pmax)&&(i < PTMAX); i++ ){
				if( cs.LoadTextLoop(bf, 1023) == FALSE ) goto _ex;
				p = StrDlm(t, bf);	// RR
				Calc(ant.pdef[i].RR, t);
				p = StrDlm(t, p);	// Type
				CalcU(ant.pdef[i].Type, t);
				for( j = 0; j < PPMAX; j++ ){
					p = StrDlm(t, p);
					if( !*t ) break;
					Calc(ant.pdef[i].L[j], t);
					p = StrDlm(t, p);
					Calc(ant.pdef[i].R[j], t);
				}
				for( ; j < PPMAX; j++ ){
					ant.pdef[i].R[j] = 0.007;
				}
			}
			AdjPdef(&ant);
		}
		else if( !strncmp(bf, "###", 3) ){		// コメント
			while(1){
				if( cs.LoadText(bf, 1023) == FALSE ) goto _ex;
				antRem += bf;
				antRem += "\r\n";
			}
		}
	}
_ex:;
	StripWireNotes(&ant, antRem);
	if( ant.fq ){
		sprintf( bf, "%.3lf", ant.fq );
	}
	else {
		bf[0] = 0;
	}
	DoFreq->Text = bf;
	ant.Edit = 0;
	ant.Flag = 1;

	exeenv.AntXc = exeenv.AntYc = 0;
}
