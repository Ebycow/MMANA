// Minimal stand-ins for the C++Builder / Win32 types the extracted MMANA code
// refers to, so it compiles with clang on macOS/Linux without modification.
#ifndef VclShimH
#define VclShimH

#include <stdio.h>
#include <stdlib.h>
#include <stdarg.h>
#include <string.h>
#include <ctype.h>
#include <float.h>
#include <math.h>
#include <strings.h>

typedef unsigned int UINT;
typedef unsigned char BYTE;
typedef unsigned char UCHAR;
typedef unsigned short USHORT;
typedef const char *LPCSTR;
typedef char *LPSTR;

#ifndef TRUE
#define TRUE 1
#endif
#ifndef FALSE
#define FALSE 0
#endif
#define MAXDOUBLE DBL_MAX

// Only referenced by unused overloads (CTextString(AnsiString&), StripWireNotes).
class AnsiString { public: const char *c_str(void) const { return ""; } };
class TCanvas;
class TMemo;

static inline char *strupr(char *s)
{
	for( char *p = s; *p; p++ ) *p = (char)toupper((unsigned char)*p);
	return s;
}
#define strcmpi strcasecmp

void WarningMB(LPCSTR fmt, ...);
LPCSTR StrDbl(double d);
LPCSTR StrCLX(double r, double x);
void GetSegPosReal(double &x, double &y, double &z, int Plus);

#endif
