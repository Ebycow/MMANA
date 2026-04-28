//Copyright+LGPL
#ifndef AntViewH
#define AntViewH

#include <Graphics.hpp>
#include "ComLib.h"

enum {
	ANT_DRAW_PLANE_XY = 0,
	ANT_DRAW_PLANE_XZ,
	ANT_DRAW_PLANE_YZ
};

enum {
	ANT_GIZMO_AXIS_X = 0,
	ANT_GIZMO_AXIS_Y,
	ANT_GIZMO_AXIS_Z
};

struct TAntViewParams {
	int Width;
	int Height;
	double CenterX;
	double CenterY;
	int Deg;
	int ZDeg;
	double Scale;
	double UnitScale;
};

struct TAntDrawPlaneState {
	int Plane;
	int Active;
	double X1;
	double Y1;
	double Z1;
};

double AntViewScaleFromTrack(int Pos);
double AntViewUnitScale(int RmdSel, int MmSel);
int AntViewRoundMouseAngle(double Value);
double AntViewMouseDragToAngle(int Pixels, int Extent);
void AntViewProject(double &x, double &y, double deg, double zdeg, double UnitScale,
	double X, double Y, double Z);
void AntViewWorldToScreen(const TAntViewParams &View, double WX, double WY, double WZ,
	int &X, int &Y);
int AntViewScreenToPlane(const TAntViewParams &View, const TAntDrawPlaneState &Plane,
	int X, int Y, double &WX, double &WY, double &WZ);
double AntViewSqDistPointToSegment(int PX, int PY, int X1, int Y1, int X2, int Y2);
void AntViewDrawScreenArrow(TCanvas *Canvas, int X1, int Y1, int X2, int Y2);
int AntViewGizmoAxisScreen(const TAntViewParams &View, double WX, double WY, double WZ,
	int Axis, int Len, int &X1, int &Y1, int &X2, int &Y2, double &DX, double &DY);
int AntViewFindSnapPoint(const ANTDEF *ap, const TAntViewParams &View, int X, int Y,
	double &WX, double &WY, double &WZ);
int AntViewFindSnapVertex(const ANTDEF *ap, const TAntViewParams &View,
	const int *Selected, int SelectionCount, int CurrentRow, int X, int Y,
	double &WX, double &WY, double &WZ);
int AntViewFindSnapEdge(const ANTDEF *ap, const TAntViewParams &View,
	const int *Selected, int SelectionCount, int CurrentRow, int X, int Y,
	double &WX, double &WY, double &WZ);

#endif
