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

//---------------------------------------------------------------------------
#include <vcl.h>
#pragma hdrstop
#include <stdio.h>
#include <string.h>

//---------------------------------------------------------------------------
USEFORM("Main.cpp", MainWnd);
USEUNIT("mininec3.cpp");
USERES("mmana.res");
USEUNIT("ComLib.cpp");
USEFORM("TextEdit.cpp", TextEditDlg);
USEFORM("VerDsp.cpp", VerDspDlg);
USEFORM("MediaDlg.cpp", MediaDlgBox);
USEFORM("ResCmp.cpp", ResCmpDlg);
USEFORM("GrpWire.cpp", GrpWireDlg);
USEFORM("ACalBox.cpp", ACalDlg);
USEFORM("ACalRes.cpp", ACalResDlg);
USEFORM("OptDlg.cpp", OptDlgBox);
USEFORM("ACalMult.cpp", ACalMultDlg);
USEFORM("WireScl.cpp", WireScaleDlg);
USEFORM("BwDisp.cpp", BwDispDlg);
USEFORM("ValRep.cpp", ValRepDlg);
USEFORM("MoveDlg.cpp", MoveDlgBox);
USEFORM("NearSet.cpp", NearSetDlg);
USEFORM("FarSet.cpp", FarSetDlg);
USEFORM("RotWire.cpp", RotWireDlg);
USEFORM("WireCad.cpp", WireCadDlg);
USEFORM("WireEdit.cpp", WireEditDlg);
USEFORM("ACalInfo.cpp", ACalInfoBox);
USERC("Mmanam.rc");
USEFORM("StackDlg.cpp", StackDlgBox);
USEFORM("WComb.cpp", WCombDlg);
USEFORM("WcombDsp.cpp", WCombDspDlg);
USEFORM("ACalEle.cpp", ACalEleBox);
USEFORM("FreqSet.cpp", FreqSetDlg);
USEFORM("Print.cpp", PrintDlgBox);
//---------------------------------------------------------------------------
typedef enum _MMANA_MINIDUMP_TYPE {
	MmanaMiniDumpNormal = 0x00000000,
	MmanaMiniDumpWithDataSegs = 0x00000001,
	MmanaMiniDumpWithHandleData = 0x00000004,
	MmanaMiniDumpWithIndirectlyReferencedMemory = 0x00000040
} MMANA_MINIDUMP_TYPE;

typedef struct _MMANA_MINIDUMP_EXCEPTION_INFORMATION {
	DWORD ThreadId;
	PEXCEPTION_POINTERS ExceptionPointers;
	BOOL ClientPointers;
} MMANA_MINIDUMP_EXCEPTION_INFORMATION, *PMMANA_MINIDUMP_EXCEPTION_INFORMATION;

typedef BOOL (WINAPI *MMANA_MINIDUMP_WRITE_DUMP)(HANDLE, DWORD, HANDLE,
	MMANA_MINIDUMP_TYPE, PMMANA_MINIDUMP_EXCEPTION_INFORMATION, void *, void *);

static volatile LONG CrashDumpWritten = 0;
static HMODULE MmanaDbgHelp = NULL;
static MMANA_MINIDUMP_WRITE_DUMP MmanaMiniDumpWriteDump = NULL;

static void __fastcall AppendTextFile(const char *path, const char *text)
{
	HANDLE file = CreateFileA(path, GENERIC_WRITE, FILE_SHARE_READ, NULL,
		OPEN_ALWAYS, FILE_ATTRIBUTE_NORMAL, NULL);
	if( file == INVALID_HANDLE_VALUE ) return;
	SetFilePointer(file, 0, NULL, FILE_END);
	DWORD len = lstrlenA(text);
	DWORD written = 0;
	WriteFile(file, text, len, &written, NULL);
	CloseHandle(file);
}

static void __fastcall GetCrashDumpPath(char *dumpDir, char *dumpPath)
{
	char exePath[MAX_PATH];
	GetModuleFileNameA(NULL, exePath, sizeof(exePath));
	char *slash = strrchr(exePath, '\\');
	if( slash != NULL ){
		*slash = '\0';
		sprintf(dumpDir, "%s\\CrashDumps", exePath);
	}
	else {
		strcpy(dumpDir, "CrashDumps");
	}
	CreateDirectoryA(dumpDir, NULL);

	SYSTEMTIME st;
	GetLocalTime(&st);
	sprintf(dumpPath, "%s\\Mmana_%04u%02u%02u_%02u%02u%02u_%lu.dmp",
		dumpDir, st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute,
		st.wSecond, GetCurrentProcessId());
}

static void __fastcall WriteCrashLog(const char *dumpDir, EXCEPTION_POINTERS *ExceptionInfo,
	DWORD gle, BOOL written, const char *dumpPath)
{
	char logPath[MAX_PATH];
	sprintf(logPath, "%s\\crash.log", dumpDir);
	SYSTEMTIME st;
	GetLocalTime(&st);
	DWORD code = 0;
	void *addr = NULL;
	if( ExceptionInfo != NULL && ExceptionInfo->ExceptionRecord != NULL ){
		code = ExceptionInfo->ExceptionRecord->ExceptionCode;
		addr = ExceptionInfo->ExceptionRecord->ExceptionAddress;
	}
	char line[1024];
	sprintf(line, "%04u/%02u/%02u %02u:%02u:%02u pid=%lu code=0x%08lx addr=%p dump=%s gle=%lu path=%s\r\n",
		st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond,
		GetCurrentProcessId(), code, addr, written ? "ok" : "failed", gle, dumpPath);
	AppendTextFile(logPath, line);
}

static void __fastcall WriteStartupLog(const char *text)
{
	char dumpDir[MAX_PATH];
	char dumpPath[MAX_PATH];
	GetCrashDumpPath(dumpDir, dumpPath);
	char logPath[MAX_PATH];
	sprintf(logPath, "%s\\startup.log", dumpDir);
	SYSTEMTIME st;
	GetLocalTime(&st);
	char line[512];
	sprintf(line, "%04u/%02u/%02u %02u:%02u:%02u pid=%lu %s\r\n",
		st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond,
		GetCurrentProcessId(), text);
	AppendTextFile(logPath, line);
}

static void __fastcall InitCrashDumpSupport(void)
{
	if( MmanaDbgHelp != NULL ) return;
	MmanaDbgHelp = LoadLibraryA("dbghelp.dll");
	if( MmanaDbgHelp != NULL ){
		MmanaMiniDumpWriteDump =
			(MMANA_MINIDUMP_WRITE_DUMP)GetProcAddress(MmanaDbgHelp, "MiniDumpWriteDump");
	}
}

static LONG WINAPI MmanaUnhandledExceptionFilter(EXCEPTION_POINTERS *ExceptionInfo)
{
	if( InterlockedExchange((LONG *)&CrashDumpWritten, 1) != 0 ){
		return EXCEPTION_EXECUTE_HANDLER;
	}
	char dumpDir[MAX_PATH];
	char dumpPath[MAX_PATH];
	GetCrashDumpPath(dumpDir, dumpPath);
	WriteCrashLog(dumpDir, ExceptionInfo, 0, FALSE, "unhandled-enter");

	BOOL written = FALSE;
	DWORD gle = 0;
	if( MmanaMiniDumpWriteDump != NULL ){
		HANDLE file = CreateFileA(dumpPath, GENERIC_WRITE, 0, NULL,
			CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, NULL);
		if( file != INVALID_HANDLE_VALUE ){
			MMANA_MINIDUMP_EXCEPTION_INFORMATION mei;
			mei.ThreadId = GetCurrentThreadId();
			mei.ExceptionPointers = ExceptionInfo;
			mei.ClientPointers = FALSE;
			written = MmanaMiniDumpWriteDump(GetCurrentProcess(), GetCurrentProcessId(),
				file, (MMANA_MINIDUMP_TYPE)(MmanaMiniDumpWithDataSegs |
				MmanaMiniDumpWithHandleData |
				MmanaMiniDumpWithIndirectlyReferencedMemory), &mei, NULL, NULL);
			if( !written ) gle = GetLastError();
			CloseHandle(file);
		}
		else {
			gle = GetLastError();
		}
	}
	else {
		gle = ERROR_PROC_NOT_FOUND;
	}
	WriteCrashLog(dumpDir, ExceptionInfo, gle, written, dumpPath);
	return EXCEPTION_EXECUTE_HANDLER;
}

static LONG WINAPI MmanaVectoredExceptionHandler(EXCEPTION_POINTERS *ExceptionInfo)
{
	if( ExceptionInfo == NULL || ExceptionInfo->ExceptionRecord == NULL ){
		return EXCEPTION_CONTINUE_SEARCH;
	}
	DWORD code = ExceptionInfo->ExceptionRecord->ExceptionCode;
	if( code != EXCEPTION_ACCESS_VIOLATION && code != 0xC0000374 ){
		return EXCEPTION_CONTINUE_SEARCH;
	}
	if( InterlockedExchange((LONG *)&CrashDumpWritten, 1) != 0 ){
		return EXCEPTION_CONTINUE_SEARCH;
	}

	char dumpDir[MAX_PATH];
	char dumpPath[MAX_PATH];
	GetCrashDumpPath(dumpDir, dumpPath);
	WriteCrashLog(dumpDir, ExceptionInfo, 0, FALSE, "vectored-enter");

	BOOL written = FALSE;
	DWORD gle = 0;
	if( MmanaMiniDumpWriteDump != NULL ){
		HANDLE file = CreateFileA(dumpPath, GENERIC_WRITE, 0, NULL,
			CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, NULL);
		if( file != INVALID_HANDLE_VALUE ){
			MMANA_MINIDUMP_EXCEPTION_INFORMATION mei;
			mei.ThreadId = GetCurrentThreadId();
			mei.ExceptionPointers = ExceptionInfo;
			mei.ClientPointers = FALSE;
			written = MmanaMiniDumpWriteDump(GetCurrentProcess(), GetCurrentProcessId(),
				file, (MMANA_MINIDUMP_TYPE)(MmanaMiniDumpWithDataSegs |
				MmanaMiniDumpWithHandleData |
				MmanaMiniDumpWithIndirectlyReferencedMemory), &mei, NULL, NULL);
			if( !written ) gle = GetLastError();
			CloseHandle(file);
		}
		else {
			gle = GetLastError();
		}
	}
	else {
		gle = ERROR_PROC_NOT_FOUND;
	}
	WriteCrashLog(dumpDir, ExceptionInfo, gle, written, dumpPath);
	return EXCEPTION_CONTINUE_SEARCH;
}

WINAPI WinMain(HINSTANCE, HINSTANCE, LPSTR, int)
{
	InitCrashDumpSupport();
	WriteStartupLog("winmain-enter");
	AddVectoredExceptionHandler(1, MmanaVectoredExceptionHandler);
	SetUnhandledExceptionFilter(MmanaUnhandledExceptionFilter);
	__try
	{
		try
		{
			Application->Initialize();
			SetUnhandledExceptionFilter(MmanaUnhandledExceptionFilter);
			Application->Title = "MMANA";
			Application->CreateForm(__classid(TMainWnd), &MainWnd);
			SetUnhandledExceptionFilter(MmanaUnhandledExceptionFilter);
			Application->Run();
		}
		catch (Exception &exception)
		{
			Application->ShowException(&exception);
		}
	}
	__except(MmanaUnhandledExceptionFilter(GetExceptionInformation()))
	{
	}
	return 0;
}
//---------------------------------------------------------------------------
