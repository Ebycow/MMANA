//Copyright+LGPL
#include <vcl.h>
#pragma hdrstop

#include "AntView.h"
#include "AntEditor.h"

#include <math.h>

//---------------------------------------------------------------------------
double AntViewScaleFromTrack(int Pos)
{
	double sc = double(Pos) / 20.0;
	return sc * sc * sc * sc;
}

//---------------------------------------------------------------------------
double AntViewUnitScale(int RmdSel, int MmSel)
{
	if( !RmdSel && MmSel ) return 1000.0;
	return 1.0;
}

//---------------------------------------------------------------------------
int AntViewRoundMouseAngle(double Value)
{
	return (Value >= 0.0) ? int(Value + 0.5) : int(Value - 0.5);
}

//---------------------------------------------------------------------------
double AntViewMouseDragToAngle(int Pixels, int Extent)
{
	if( Extent < 240 ) Extent = 240;
	if( Extent > 480 ) Extent = 480;
	return double(Pixels) * 180.0 / double(Extent);
}

//---------------------------------------------------------------------------
void AntViewProject(double &x, double &y, double deg, double zdeg, double UnitScale,
	double X, double Y, double Z)
{
	Calc3DXY(x, y, deg, zdeg, X, Y, Z);
	x *= UnitScale;
	y *= UnitScale;
}

//---------------------------------------------------------------------------
void AntViewWorldToScreen(const TAntViewParams &View, double WX, double WY, double WZ,
	int &X, int &Y)
{
	int Xc = int(View.Width / 2 + (View.CenterX * View.Scale));
	int Yc = int(View.Height / 2 + (View.CenterY * View.Scale));
	double deg = double(View.Deg) * (PAI / 180.0);
	double zdeg = double(View.ZDeg) * (PAI / 180.0);
	double x, y;
	AntViewProject(x, y, deg, zdeg, View.UnitScale, WX, WY, WZ);
	X = int(x * View.Scale) + Xc;
	Y = Yc - int(y * View.Scale);
}

//---------------------------------------------------------------------------
int AntViewScreenToPlane(const TAntViewParams &View, const TAntDrawPlaneState &Plane,
	int X, int Y, double &WX, double &WY, double &WZ)
{
	if( (View.Scale <= 0.0) || (View.UnitScale <= 0.0) ) return FALSE;

	int Xc = int(View.Width / 2 + (View.CenterX * View.Scale));
	int Yc = int(View.Height / 2 + (View.CenterY * View.Scale));
	double sx = (double(X - Xc) / View.Scale) / View.UnitScale;
	double sy = (double(Yc - Y) / View.Scale) / View.UnitScale;
	double deg = double(View.Deg) * (PAI / 180.0);
	double zdeg = double(View.ZDeg) * (PAI / 180.0);
	double sd = sin(deg);
	double cd = cos(deg);
	double sz = sin(zdeg);
	double cz = cos(zdeg);
	double planeX = Plane.Active ? Plane.X1 : 0.0;
	double planeY = Plane.Active ? Plane.Y1 : 0.0;
	double planeZ = Plane.Active ? Plane.Z1 : 0.0;

	switch( Plane.Plane ){
		case ANT_DRAW_PLANE_XY:
			if( ABS(sz) < 0.05 ) return FALSE;
			{
				double q = (sy - (planeZ * cz)) / sz;
				WX = (sd * sx) - (cd * q);
				WY = (cd * sx) + (sd * q);
				WZ = planeZ;
			}
			return TRUE;

		case ANT_DRAW_PLANE_XZ:
			if( (ABS(sd) < 0.05) || (ABS(cz) < 0.05) ) return FALSE;
			WX = (sx - (cd * planeY)) / sd;
			WY = planeY;
			WZ = (sy - (sd * sz * planeY) + (cd * sz * WX)) / cz;
			return TRUE;

		case ANT_DRAW_PLANE_YZ:
			if( (ABS(cd) < 0.05) || (ABS(cz) < 0.05) ) return FALSE;
			WX = planeX;
			WY = (sx - (sd * planeX)) / cd;
			WZ = (sy + (cd * sz * planeX) - (sd * sz * WY)) / cz;
			return TRUE;
	}
	return FALSE;
}

//---------------------------------------------------------------------------
double AntViewSqDistPointToSegment(int PX, int PY, int X1, int Y1, int X2, int Y2)
{
	double vx = double(X2 - X1);
	double vy = double(Y2 - Y1);
	double wx = double(PX - X1);
	double wy = double(PY - Y1);
	double len2 = (vx * vx) + (vy * vy);
	double t = 0.0;
	if( len2 > 0.0 ){
		t = ((wx * vx) + (wy * vy)) / len2;
		if( t < 0.0 ) t = 0.0;
		else if( t > 1.0 ) t = 1.0;
	}
	double dx = double(X1) + (vx * t) - double(PX);
	double dy = double(Y1) + (vy * t) - double(PY);
	return (dx * dx) + (dy * dy);
}

//---------------------------------------------------------------------------
void AntViewDrawScreenArrow(TCanvas *Canvas, int X1, int Y1, int X2, int Y2)
{
	Canvas->MoveTo(X1, Y1);
	Canvas->LineTo(X2, Y2);
	double dx = double(X2 - X1);
	double dy = double(Y2 - Y1);
	double len = sqrt((dx * dx) + (dy * dy));
	if( len < 1.0 ) return;
	dx /= len;
	dy /= len;
	double px = -dy;
	double py = dx;
	int ax1 = int(double(X2) - (dx * 8.0) + (px * 4.0));
	int ay1 = int(double(Y2) - (dy * 8.0) + (py * 4.0));
	int ax2 = int(double(X2) - (dx * 8.0) - (px * 4.0));
	int ay2 = int(double(Y2) - (dy * 8.0) - (py * 4.0));
	Canvas->MoveTo(X2, Y2);
	Canvas->LineTo(ax1, ay1);
	Canvas->MoveTo(X2, Y2);
	Canvas->LineTo(ax2, ay2);
}

//---------------------------------------------------------------------------
int AntViewGizmoAxisScreen(const TAntViewParams &View, double WX, double WY, double WZ,
	int Axis, int Len, int &X1, int &Y1, int &X2, int &Y2, double &DX, double &DY)
{
	double ex = WX;
	double ey = WY;
	double ez = WZ;
	switch( Axis ){
		case ANT_GIZMO_AXIS_X: ex += 1.0; break;
		case ANT_GIZMO_AXIS_Y: ey += 1.0; break;
		case ANT_GIZMO_AXIS_Z: ez += 1.0; break;
		default: return FALSE;
	}
	int AX, AY;
	AntViewWorldToScreen(View, WX, WY, WZ, X1, Y1);
	AntViewWorldToScreen(View, ex, ey, ez, AX, AY);
	DX = double(AX - X1);
	DY = double(AY - Y1);
	double d = sqrt((DX * DX) + (DY * DY));
	if( d < 0.1 ) return FALSE;
	X2 = X1 + int((DX / d) * double(Len));
	Y2 = Y1 + int((DY / d) * double(Len));
	return TRUE;
}

//---------------------------------------------------------------------------
int AntViewFindSnapPoint(const ANTDEF *ap, const TAntViewParams &View, int X, int Y,
	double &WX, double &WY, double &WZ)
{
	int found = FALSE;
	double best = (10.0 * 10.0) + 1.0;
	for( int i = 0; i < ap->wmax; i++ ){
		const WDEF *wp = &ap->wdef[i];
		for( int p = 0; p < 3; p++ ){
			double tx = wp->X1;
			double ty = wp->Y1;
			double tz = wp->Z1;
			if( p == 1 ){
				tx = wp->X2;
				ty = wp->Y2;
				tz = wp->Z2;
			}
			else if( p == 2 ){
				tx = (wp->X1 + wp->X2) / 2.0;
				ty = (wp->Y1 + wp->Y2) / 2.0;
				tz = (wp->Z1 + wp->Z2) / 2.0;
			}
			int sx, sy;
			AntViewWorldToScreen(View, tx, ty, tz, sx, sy);
			double dx = double(sx - X);
			double dy = double(sy - Y);
			double d = (dx * dx) + (dy * dy);
			if( d < best ){
				best = d;
				WX = tx;
				WY = ty;
				WZ = tz;
				found = TRUE;
			}
		}
	}
	return found;
}

//---------------------------------------------------------------------------
int AntViewFindSnapVertex(const ANTDEF *ap, const TAntViewParams &View,
	const int *Selected, int SelectionCount, int CurrentRow, int X, int Y,
	double &WX, double &WY, double &WZ)
{
	int found = FALSE;
	int best = (12 * 12) + 1;
	for( int i = 0; i < ap->wmax; i++ ){
		if( AntEditorIsWireSelected(ap, Selected, SelectionCount, CurrentRow, i) ) continue;
		int sx, sy;
		AntViewWorldToScreen(View, ap->wdef[i].X1, ap->wdef[i].Y1, ap->wdef[i].Z1, sx, sy);
		int dx = sx - X;
		int dy = sy - Y;
		int d = (dx * dx) + (dy * dy);
		if( d < best ){
			best = d;
			WX = ap->wdef[i].X1;
			WY = ap->wdef[i].Y1;
			WZ = ap->wdef[i].Z1;
			found = TRUE;
		}
		AntViewWorldToScreen(View, ap->wdef[i].X2, ap->wdef[i].Y2, ap->wdef[i].Z2, sx, sy);
		dx = sx - X;
		dy = sy - Y;
		d = (dx * dx) + (dy * dy);
		if( d < best ){
			best = d;
			WX = ap->wdef[i].X2;
			WY = ap->wdef[i].Y2;
			WZ = ap->wdef[i].Z2;
			found = TRUE;
		}
	}
	return found;
}

//---------------------------------------------------------------------------
int AntViewFindSnapEdge(const ANTDEF *ap, const TAntViewParams &View,
	const int *Selected, int SelectionCount, int CurrentRow, int X, int Y,
	double &WX, double &WY, double &WZ)
{
	int found = FALSE;
	double best = (12.0 * 12.0) + 1.0;
	for( int i = 0; i < ap->wmax; i++ ){
		if( AntEditorIsWireSelected(ap, Selected, SelectionCount, CurrentRow, i) ) continue;
		int x1, y1, x2, y2;
		AntViewWorldToScreen(View, ap->wdef[i].X1, ap->wdef[i].Y1, ap->wdef[i].Z1, x1, y1);
		AntViewWorldToScreen(View, ap->wdef[i].X2, ap->wdef[i].Y2, ap->wdef[i].Z2, x2, y2);
		double vx = double(x2 - x1);
		double vy = double(y2 - y1);
		double len2 = (vx * vx) + (vy * vy);
		double t = 0.0;
		if( len2 > 0.0 ){
			t = ((double(X - x1) * vx) + (double(Y - y1) * vy)) / len2;
			if( t < 0.0 ) t = 0.0;
			else if( t > 1.0 ) t = 1.0;
		}
		double px = double(x1) + (vx * t);
		double py = double(y1) + (vy * t);
		double dx = px - double(X);
		double dy = py - double(Y);
		double d = (dx * dx) + (dy * dy);
		if( d < best ){
			best = d;
			WX = ap->wdef[i].X1 + ((ap->wdef[i].X2 - ap->wdef[i].X1) * t);
			WY = ap->wdef[i].Y1 + ((ap->wdef[i].Y2 - ap->wdef[i].Y1) * t);
			WZ = ap->wdef[i].Z1 + ((ap->wdef[i].Z2 - ap->wdef[i].Z1) * t);
			found = TRUE;
		}
	}
	return found;
}
